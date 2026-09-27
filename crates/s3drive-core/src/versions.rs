//! バージョン管理（04 §11）。

use aws_sdk_s3::types::EncodingType;

use crate::Core;
use crate::aws::error_map::{self, Ctx};
use crate::connections::ConnCtx;
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::{BatchResult, ObjectVersion, StorageClass, VersionPage};
use crate::objects::{self, head};
use crate::search::{self, IndexedObject};
use crate::util::{key, time};

/// 1 回に返すバージョンの件数（03 §5.6 の「100 件ずつ表示」）。
pub const VERSION_PAGE_SIZE: usize = 100;

/// `ListObjectVersions` の 1 ページ分を、キーが完全に一致するものだけに絞って返す（04 §11.2）。
async fn list_page(
    ctx: &ConnCtx,
    object_key: &str,
    key_marker: Option<String>,
    version_marker: Option<String>,
) -> CoreResult<(Vec<ObjectVersion>, Option<(String, String)>)> {
    let out = ctx
        .clients
        .s3
        .list_object_versions()
        .bucket(&ctx.bucket)
        .prefix(object_key)
        .max_keys(1000)
        .encoding_type(EncodingType::Url)
        .set_key_marker(key_marker)
        .set_version_id_marker(version_marker)
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucketVersions")))?;
    let mut versions: Vec<ObjectVersion> = out
        .versions()
        .iter()
        .filter(|v| v.key().map(key::url_decode_key).as_deref() == Some(object_key))
        .map(|v| ObjectVersion {
            version_id: v.version_id().unwrap_or("null").to_string(),
            is_latest: v.is_latest() == Some(true),
            is_delete_marker: false,
            last_modified: v
                .last_modified()
                .map(time::aws_to_rfc3339)
                .unwrap_or_default(),
            size: Some(v.size().unwrap_or(0).max(0) as u64),
            storage_class: Some(StorageClass::from_s3(v.storage_class().map(|c| c.as_str()))),
            etag: Some(objects::trim_etag(v.e_tag())),
        })
        .collect();
    versions.extend(
        out.delete_markers()
            .iter()
            .filter(|m| m.key().map(key::url_decode_key).as_deref() == Some(object_key))
            .map(|m| ObjectVersion {
                version_id: m.version_id().unwrap_or("null").to_string(),
                is_latest: m.is_latest() == Some(true),
                is_delete_marker: true,
                last_modified: m
                    .last_modified()
                    .map(time::aws_to_rfc3339)
                    .unwrap_or_default(),
                size: None,
                storage_class: None,
                etag: None,
            }),
    );
    // 返ってきたキーが対象を過ぎていれば、それ以上読む必要はない
    let passed = out
        .versions()
        .iter()
        .filter_map(|v| v.key())
        .chain(out.delete_markers().iter().filter_map(|m| m.key()))
        .any(|k| key::url_decode_key(k).as_str() > object_key);
    let next = if out.is_truncated() == Some(true) && !passed {
        match (out.next_key_marker(), out.next_version_id_marker()) {
            (Some(k), Some(v)) => Some((k.to_string(), v.to_string())),
            _ => None,
        }
    } else {
        None
    };
    Ok((versions, next))
}

fn sort_newest_first(versions: &mut [ObjectVersion]) {
    versions.sort_by(|a, b| {
        b.last_modified
            .cmp(&a.last_modified)
            .then(b.is_latest.cmp(&a.is_latest))
    });
}

/// 全バージョンと削除マーカーの（キー、バージョン ID）。`exact` を指定するとそのキーだけに絞る。
pub(crate) async fn list_all_versions(
    ctx: &ConnCtx,
    prefix: &str,
    exact: Option<&str>,
) -> CoreResult<Vec<(String, Option<String>)>> {
    let mut key_marker = None;
    let mut version_marker = None;
    let mut out_ids = Vec::new();
    loop {
        let out = ctx
            .clients
            .s3_bulk
            .list_object_versions()
            .bucket(&ctx.bucket)
            .prefix(prefix)
            .max_keys(1000)
            .encoding_type(EncodingType::Url)
            .set_key_marker(key_marker)
            .set_version_id_marker(version_marker)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucketVersions")))?;
        let items = out
            .versions()
            .iter()
            .map(|v| (v.key(), v.version_id()))
            .chain(
                out.delete_markers()
                    .iter()
                    .map(|m| (m.key(), m.version_id())),
            );
        for (k, v) in items {
            let Some(k) = k.map(key::url_decode_key) else {
                continue;
            };
            if exact.is_some_and(|e| e != k) {
                continue;
            }
            out_ids.push((k, v.map(str::to_string)));
        }
        if out.is_truncated() != Some(true) {
            return Ok(out_ids);
        }
        key_marker = out.next_key_marker().map(str::to_string);
        version_marker = out.next_version_id_marker().map(str::to_string);
    }
}

/// 最も古いバージョンの日時（作成日の優先 1。04 §9.2）。
pub(crate) async fn oldest_version_date(
    ctx: &ConnCtx,
    object_key: &str,
) -> CoreResult<Option<String>> {
    let mut all = Vec::new();
    let mut cursor = None;
    for _ in 0..10 {
        let (mut page, next) = match cursor.take() {
            None => list_page(ctx, object_key, None, None).await?,
            Some((k, v)) => list_page(ctx, object_key, Some(k), Some(v)).await?,
        };
        all.append(&mut page);
        match next {
            Some(n) => cursor = Some(n),
            None => break,
        }
    }
    Ok(all
        .into_iter()
        .filter(|v| !v.is_delete_marker)
        .map(|v| v.last_modified)
        .min())
}

fn encode_cursor(key_marker: &str, version_marker: &str, skip: usize) -> String {
    serde_json::json!({ "k": key_marker, "v": version_marker, "s": skip }).to_string()
}

pub struct VersionService<'a> {
    core: &'a Core,
}

impl Core {
    pub fn versions(&self) -> VersionService<'_> {
        VersionService { core: self }
    }
}

impl VersionService<'_> {
    /// バージョン一覧（新しい順、100 件ずつ）。
    pub async fn list(
        &self,
        connection_id: &str,
        object_key: &str,
        cursor: Option<String>,
    ) -> CoreResult<VersionPage> {
        key::validate_key(object_key)?;
        let ctx = self.core.ctx(connection_id).await?;
        let (mut key_marker, mut version_marker, skip) = match cursor.as_deref() {
            Some(c) => {
                let v: serde_json::Value =
                    serde_json::from_str(c).map_err(|_| CoreError::internal("invalid cursor"))?;
                (
                    v["k"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                    v["v"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                    v["s"].as_u64().unwrap_or(0) as usize,
                )
            }
            None => (None, None, 0),
        };
        let start_markers = (key_marker.clone(), version_marker.clone());
        // 1 ページ（1,000 件）の中で 100 件ずつ返し、読み終えたら次のページに進む
        let (mut versions, next) =
            list_page(&ctx, object_key, key_marker.take(), version_marker.take()).await?;
        sort_newest_first(&mut versions);
        let page: Vec<ObjectVersion> = versions
            .iter()
            .skip(skip)
            .take(VERSION_PAGE_SIZE)
            .cloned()
            .collect();
        let next_cursor = if skip + VERSION_PAGE_SIZE < versions.len() {
            Some(encode_cursor(
                start_markers.0.as_deref().unwrap_or(""),
                start_markers.1.as_deref().unwrap_or(""),
                skip + VERSION_PAGE_SIZE,
            ))
        } else {
            next.map(|(k, v)| encode_cursor(&k, &v, 0))
        };
        Ok(VersionPage {
            versions: page,
            next_cursor,
        })
    }

    /// 以前のバージョンを新しい最新バージョンとしてコピーする（04 §11.3）。
    pub async fn restore(
        &self,
        connection_id: &str,
        object_key: &str,
        version_id: &str,
    ) -> CoreResult<ObjectVersion> {
        key::validate_key(object_key)?;
        let ctx = self.core.ctx(connection_id).await?;
        let source = head(&ctx, object_key, Some(version_id)).await?;
        if source.restore.needs_restore() {
            return Err(CoreError::new(ErrorCode::InvalidObjectState));
        }
        objects_copy(&ctx, object_key, version_id, &source).await?;
        let latest = head(&ctx, object_key, None).await?;
        search::upsert(
            &self.core.0.db,
            connection_id,
            vec![IndexedObject {
                key: object_key.to_string(),
                size: latest.size,
                last_modified: latest.last_modified.clone(),
                etag: Some(latest.etag.clone()),
                storage_class: latest.storage_class,
            }],
        )
        .await?;
        Ok(ObjectVersion {
            version_id: latest.version_id.unwrap_or_else(|| "null".into()),
            is_latest: true,
            is_delete_marker: false,
            last_modified: latest.last_modified,
            size: Some(latest.size),
            storage_class: Some(latest.storage_class),
            etag: Some(latest.etag),
        })
    }

    /// バージョンの完全削除（04 §11.4）。
    pub async fn delete(
        &self,
        connection_id: &str,
        object_key: &str,
        version_id: &str,
    ) -> CoreResult<()> {
        key::validate_key(object_key)?;
        let ctx = self.core.ctx(connection_id).await?;
        ctx.clients
            .s3
            .delete_object()
            .bucket(&ctx.bucket)
            .key(object_key)
            .version_id(version_id)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:DeleteObjectVersion")))?;
        self.sync_index(&ctx, connection_id, object_key).await
    }

    /// 削除済みの項目の復元。最新の実体より新しい削除マーカーを取り除く（04 §11.5）。
    pub async fn undelete(&self, connection_id: &str, keys: &[String]) -> CoreResult<BatchResult> {
        let ctx = self.core.ctx(connection_id).await?;
        let mut result = BatchResult::default();
        for k in keys {
            key::validate_key(k)?;
            match self.undelete_one(&ctx, connection_id, k).await {
                Ok(true) => result.succeeded += 1,
                Ok(false) => result.skip(k, "削除マーカーがありません"),
                Err(e) => result.fail(k, e),
            }
        }
        Ok(result)
    }

    async fn undelete_one(
        &self,
        ctx: &ConnCtx,
        connection_id: &str,
        object_key: &str,
    ) -> CoreResult<bool> {
        let mut all = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let (mut page, next) = match cursor.take() {
                None => list_page(ctx, object_key, None, None).await?,
                Some((k, v)) => list_page(ctx, object_key, Some(k), Some(v)).await?,
            };
            all.append(&mut page);
            match next {
                Some(n) => cursor = Some(n),
                None => break,
            }
        }
        sort_newest_first(&mut all);
        let newest_data = all
            .iter()
            .find(|v| !v.is_delete_marker)
            .map(|v| v.last_modified.clone());
        let markers: Vec<String> = all
            .iter()
            .filter(|v| v.is_delete_marker)
            .filter(|v| newest_data.as_ref().is_none_or(|d| &v.last_modified >= d))
            .map(|v| v.version_id.clone())
            .collect();
        if markers.is_empty() {
            return Ok(false);
        }
        for version_id in markers {
            ctx.clients
                .s3
                .delete_object()
                .bucket(&ctx.bucket)
                .key(object_key)
                .version_id(version_id)
                .send()
                .await
                .map_err(|e| error_map::classify(&e, Ctx::Op("s3:DeleteObjectVersion")))?;
        }
        self.sync_index(ctx, connection_id, object_key).await?;
        Ok(true)
    }

    /// 最新の状態をインデックスに反映する。
    async fn sync_index(
        &self,
        ctx: &ConnCtx,
        connection_id: &str,
        object_key: &str,
    ) -> CoreResult<()> {
        match head(ctx, object_key, None).await {
            Ok(info) => {
                search::upsert(
                    &self.core.0.db,
                    connection_id,
                    vec![IndexedObject {
                        key: object_key.to_string(),
                        size: info.size,
                        last_modified: info.last_modified,
                        etag: Some(info.etag),
                        storage_class: info.storage_class,
                    }],
                )
                .await
            }
            Err(e) if e.code == ErrorCode::NotFound => {
                search::remove(&self.core.0.db, connection_id, vec![object_key.to_string()]).await
            }
            Err(e) => Err(e),
        }
    }
}

async fn objects_copy(
    ctx: &ConnCtx,
    object_key: &str,
    version_id: &str,
    source: &objects::HeadInfo,
) -> CoreResult<()> {
    objects::copy_version(ctx, object_key, version_id, source).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(id: &str, at: &str, latest: bool, marker: bool) -> ObjectVersion {
        ObjectVersion {
            version_id: id.into(),
            is_latest: latest,
            is_delete_marker: marker,
            last_modified: at.into(),
            size: None,
            storage_class: None,
            etag: None,
        }
    }

    #[test]
    fn sorts_versions_newest_first() {
        let mut vs = vec![
            v("a", "2026-01-01T00:00:00Z", false, false),
            v("c", "2026-03-01T00:00:00Z", true, true),
            v("b", "2026-02-01T00:00:00Z", false, false),
        ];
        sort_newest_first(&mut vs);
        let ids: Vec<&str> = vs.iter().map(|v| v.version_id.as_str()).collect();
        assert_eq!(ids, ["c", "b", "a"]);
    }

    #[test]
    fn encodes_cursors() {
        let c = encode_cursor("k", "v", 100);
        let parsed: serde_json::Value = serde_json::from_str(&c).unwrap();
        assert_eq!(parsed["s"], 100);
    }
}
