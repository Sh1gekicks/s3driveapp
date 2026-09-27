//! アーカイブの取り出し（04 §8.4）。

use aws_sdk_s3::types::{GlacierJobParameters, RestoreRequest};
use futures::{StreamExt, stream};
use rusqlite::params;

use super::{ObjectService, head, list_recursive};
use crate::aws::error_map::{self, Ctx};
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::{
    BatchResult, RestoreCompleted, RestoreState, RestoreTier, StorageClass, Target,
};
use crate::util::{key, time};

/// HeadObject の `x-amz-restore` を解釈する。`archived` はアーカイブ（取り出しが必要なクラス・階層）か。
pub fn parse_restore_header(header: Option<&str>, archived: bool) -> RestoreState {
    let Some(h) = header else {
        return if archived {
            RestoreState::Archived
        } else {
            RestoreState::NotArchived
        };
    };
    if h.contains("ongoing-request=\"true\"") {
        return RestoreState::InProgress;
    }
    let expiry = h
        .split("expiry-date=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next());
    match expiry.and_then(time::parse_http_date) {
        Some(dt) => RestoreState::Restored {
            expiry: time::to_rfc3339(dt),
        },
        None if archived => RestoreState::Archived,
        None => RestoreState::NotArchived,
    }
}

impl ObjectService<'_> {
    /// 取り出しを要求する。フォルダは配下のアーカイブを対象にする。
    pub async fn request_restore(
        &self,
        connection_id: &str,
        targets: &[Target],
        tier: RestoreTier,
        days: Option<u32>,
    ) -> CoreResult<BatchResult> {
        let ctx = self.core.ctx(connection_id).await?;
        let days = days.map(|d| d.clamp(1, 30));
        let mut keys: Vec<(String, Option<String>)> = Vec::new();
        for t in targets {
            key::validate_key(&t.key)?;
            if t.is_folder {
                let (items, _) = list_recursive(&ctx, &t.key, usize::MAX).await?;
                keys.extend(
                    items
                        .into_iter()
                        .filter(|o| {
                            o.storage_class.is_archive()
                                || o.storage_class == StorageClass::IntelligentTiering
                        })
                        .map(|o| (o.key, None)),
                );
            } else {
                keys.push((t.key.clone(), t.version_id.clone()));
            }
        }

        let results: Vec<(String, Option<String>, CoreResult<bool>)> = stream::iter(keys)
            .map(|(k, v)| {
                let ctx = ctx.clone();
                async move {
                    let r = request_one(&ctx, &k, v.as_deref(), tier, days).await;
                    (k, v, r)
                }
            })
            .buffer_unordered(8)
            .collect()
            .await;

        let mut result = BatchResult::default();
        let mut requested = Vec::new();
        for (k, v, r) in results {
            match r {
                Ok(true) => {
                    result.succeeded += 1;
                    requested.push((k, v));
                }
                Ok(false) => result.skip(k, "アーカイブではないため取り出す必要はありません"),
                Err(e) if e.code == ErrorCode::RestoreInProgress => {
                    result.succeeded += 1;
                    requested.push((k, v));
                }
                Err(e) => result.fail(k, e),
            }
        }
        let cid = connection_id.to_string();
        let now = time::now_rfc3339();
        self.core
            .0
            .db
            .run(move |c| {
                let tx = c.transaction()?;
                for (k, v) in requested {
                    tx.execute(
                        "INSERT INTO restore_requests (connection_id, key, version_id, tier, days, status, requested_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, 'inProgress', ?6)
                         ON CONFLICT (connection_id, key, version_id) DO UPDATE SET tier = excluded.tier, days = excluded.days,
                           status = 'inProgress', requested_at = excluded.requested_at, expiry_at = NULL",
                        params![cid, k, v.unwrap_or_default(), tier.as_str(), days, now],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .await?;
        Ok(result)
    }

    /// 未完了の取り出し要求を HeadObject で確認し、完了したものを返す（15 分ごとに呼ぶ）。
    pub async fn check_restores(&self) -> CoreResult<Vec<RestoreCompleted>> {
        let Ok(session) = self.core.account() else {
            return Ok(Vec::new());
        };
        let own: Vec<String> = self.core.0.settings.read(|f| {
            f.accounts
                .get(&session.sub)
                .map(|a| a.connections.iter().map(|c| c.id.clone()).collect())
                .unwrap_or_default()
        });
        let pending: Vec<(String, String, String)> = self
            .core
            .0
            .db
            .run(|c| {
                let mut stmt = c.prepare(
                    "SELECT connection_id, key, version_id FROM restore_requests WHERE status = 'inProgress'",
                )?;
                let rows = stmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await?;
        let mut completed = Vec::new();
        for (cid, k, v) in pending.into_iter().filter(|(c, _, _)| own.contains(c)) {
            let Ok(ctx) = self.core.ctx(&cid).await else {
                continue;
            };
            let version = (!v.is_empty()).then_some(v.as_str());
            let state = match head(&ctx, &k, version).await {
                Ok(info) => info.restore,
                Err(e) if e.code == ErrorCode::NotFound => RestoreState::NotArchived,
                Err(e) => {
                    log::debug!("取り出し状態を確認できません: {e}");
                    continue;
                }
            };
            let (status, expiry) = match state {
                RestoreState::InProgress => continue,
                RestoreState::Restored { expiry } => ("restored", Some(expiry)),
                _ => ("restored", None),
            };
            let (c2, k2, v2) = (cid.clone(), k.clone(), v.clone());
            self.core
                .0
                .db
                .run(move |c| {
                    c.execute(
                        "UPDATE restore_requests SET status = ?4, expiry_at = ?5 WHERE connection_id = ?1 AND key = ?2 AND version_id = ?3",
                        params![c2, k2, v2, status, expiry],
                    )?;
                    Ok(())
                })
                .await?;
            completed.push(RestoreCompleted {
                connection_id: cid,
                key: k,
            });
        }
        Ok(completed)
    }
}

/// 1 件の取り出しを要求する。アーカイブでなければ `false`。
async fn request_one(
    ctx: &crate::connections::ConnCtx,
    object_key: &str,
    version_id: Option<&str>,
    tier: RestoreTier,
    days: Option<u32>,
) -> CoreResult<bool> {
    let info = head(ctx, object_key, version_id).await?;
    match info.restore {
        RestoreState::NotArchived | RestoreState::Restored { .. } => return Ok(false),
        RestoreState::InProgress => return Err(CoreError::new(ErrorCode::RestoreInProgress)),
        RestoreState::Archived => {}
    }
    let intelligent = info.storage_class == StorageClass::IntelligentTiering;
    let mut request = RestoreRequest::builder();
    if !intelligent {
        // Intelligent-Tiering のアーカイブ階層は Days を指定しない（取り出すと高頻度アクセス階層に戻る）
        request = request.days(days.unwrap_or(7) as i32);
        request = request.glacier_job_parameters(
            GlacierJobParameters::builder()
                .tier(tier.to_sdk())
                .build()
                .map_err(CoreError::internal)?,
        );
    }
    ctx.clients
        .s3
        .restore_object()
        .bucket(&ctx.bucket)
        .key(object_key)
        .set_version_id(version_id.map(str::to_string))
        .restore_request(request.build())
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Op("s3:RestoreObject")))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_restore_headers() {
        assert_eq!(parse_restore_header(None, false), RestoreState::NotArchived);
        assert_eq!(parse_restore_header(None, true), RestoreState::Archived);
        assert_eq!(
            parse_restore_header(Some("ongoing-request=\"true\""), true),
            RestoreState::InProgress
        );
        assert_eq!(
            parse_restore_header(
                Some("ongoing-request=\"false\", expiry-date=\"Fri, 21 Dec 2012 00:00:00 GMT\""),
                true
            ),
            RestoreState::Restored {
                expiry: "2012-12-21T00:00:00Z".into()
            }
        );
    }
}
