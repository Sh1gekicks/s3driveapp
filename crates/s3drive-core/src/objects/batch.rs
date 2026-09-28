//! 一括操作（削除・移動・名前の変更・クラス変更）。ジョブとして実行し、進捗をチャネルで返す（01 §6.2）。

use std::collections::BTreeSet;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use aws_sdk_s3::types::{Delete, ObjectIdentifier};
use futures::{StreamExt, stream};
use tokio_util::sync::CancellationToken;

use super::copy::{self, CopySpec};
use super::{Listed, ObjectService, head, list_recursive, remote_conflict};
use crate::Core;
use crate::aws::error_map::{self, Ctx, ErrorParts};
use crate::connections::ConnCtx;
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::jobs::{JobKind, ProgressSink};
use crate::model::{
    BatchEvent, BatchFinished, BatchProgress, BatchResult, ConflictDecision, Decisions, JobId,
    StorageClass, Target,
};
use crate::search::{self, IndexedObject};
use crate::util::key;

/// 一括操作で並列に呼ぶ API の数（01 §6.2）。
const BATCH_CONCURRENCY: usize = 8;
/// `DeleteObjects` の並列数（04 §6.2）。
const DELETE_CONCURRENCY: usize = 4;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// （キー、バージョン ID）。
type VersionedKey = (String, Option<String>);
/// （キー、エラー）。
type KeyError = (String, CoreError);

/// 間引いて進捗を送る（1 ジョブあたり最大 10 回/秒）。
pub(crate) struct BatchProgressEmitter {
    job_id: JobId,
    sink: Arc<dyn ProgressSink<BatchEvent>>,
    state: Mutex<(u64, Option<u64>, Option<Instant>)>,
}

impl BatchProgressEmitter {
    fn new(job_id: JobId, sink: Arc<dyn ProgressSink<BatchEvent>>) -> Self {
        Self {
            job_id,
            sink,
            state: Mutex::new((0, None, None)),
        }
    }

    fn set_total(&self, total: u64) {
        let mut s = self.state.lock().unwrap();
        s.1 = Some(total);
        self.emit(&mut s, true);
    }

    fn advance(&self, n: u64) {
        let mut s = self.state.lock().unwrap();
        s.0 += n;
        let force = s.1 == Some(s.0);
        self.emit(&mut s, force);
    }

    fn emit(&self, s: &mut (u64, Option<u64>, Option<Instant>), force: bool) {
        if !force && s.2.is_some_and(|t| t.elapsed() < PROGRESS_INTERVAL) {
            return;
        }
        s.2 = Some(Instant::now());
        self.sink.send(BatchEvent::Progress(BatchProgress {
            job_id: self.job_id.clone(),
            done: s.0,
            total: s.1,
        }));
    }
}

/// ジョブを登録して背景で実行する。結果は `finished` イベントで返す。
fn spawn_batch<F, Fut>(
    core: &Core,
    kind: JobKind,
    connection_id: &str,
    sink: Arc<dyn ProgressSink<BatchEvent>>,
    run: F,
) -> JobId
where
    F: FnOnce(Arc<BatchProgressEmitter>, CancellationToken) -> Fut + Send + 'static,
    Fut: Future<Output = CoreResult<BatchResult>> + Send + 'static,
{
    let (job_id, cancel) = core.jobs().register(kind, Some(connection_id));
    let progress = Arc::new(BatchProgressEmitter::new(job_id.clone(), sink.clone()));
    let core = core.clone();
    let id = job_id.clone();
    tokio::spawn(async move {
        let result = match run(progress, cancel).await {
            Ok(r) => r,
            Err(e) => {
                let mut r = BatchResult::default();
                if !e.is_canceled() {
                    r.fail("", e);
                }
                r
            }
        };
        sink.send(BatchEvent::Finished(BatchFinished {
            job_id: id.clone(),
            result,
        }));
        core.jobs().finish(&id);
    });
    job_id
}

fn check_cancel(cancel: &CancellationToken) -> CoreResult<()> {
    if cancel.is_cancelled() {
        Err(CoreError::canceled())
    } else {
        Ok(())
    }
}

/// フォルダを移動・名前変更したときの各キーの移動先（`root` 配下を `new_root` 配下に写す）。
pub fn relocation_plan(keys: &[String], root: &str, new_root: &str) -> Vec<(String, String)> {
    keys.iter()
        .filter_map(|k| {
            k.strip_prefix(root)
                .map(|rest| (k.clone(), format!("{new_root}{rest}")))
        })
        .collect()
}

impl ObjectService<'_> {
    /// 削除（04 §6）。`all_versions` はバージョニング有効時の「すべてのバージョンを完全に削除する」。
    pub async fn delete(
        &self,
        connection_id: &str,
        targets: Vec<Target>,
        all_versions: bool,
        sink: Arc<dyn ProgressSink<BatchEvent>>,
    ) -> CoreResult<JobId> {
        for t in &targets {
            key::validate_key(&t.key)?;
        }
        let ctx = self.core.ctx(connection_id).await?;
        let core = self.core.clone();
        let cid = connection_id.to_string();
        Ok(spawn_batch(
            self.core,
            JobKind::Delete,
            connection_id,
            sink,
            move |progress, cancel| async move {
                delete_job(&core, &ctx, &cid, targets, all_versions, &progress, &cancel).await
            },
        ))
    }

    /// 移動（04 §7.3）。
    pub async fn move_objects(
        &self,
        connection_id: &str,
        targets: Vec<Target>,
        dest_prefix: String,
        decisions: Decisions,
        sink: Arc<dyn ProgressSink<BatchEvent>>,
    ) -> CoreResult<JobId> {
        key::validate_prefix(&dest_prefix)?;
        for t in &targets {
            key::validate_key(&t.key)?;
            if t.is_folder && key::is_within(&dest_prefix, &t.key) {
                return Err(CoreError::with_message(
                    ErrorCode::InvalidName,
                    "フォルダを自身またはその配下へ移動することはできません",
                ));
            }
        }
        let ctx = self.core.ctx(connection_id).await?;
        let core = self.core.clone();
        let cid = connection_id.to_string();
        Ok(spawn_batch(
            self.core,
            JobKind::Move,
            connection_id,
            sink,
            move |progress, cancel| async move {
                let mut result = BatchResult::default();
                let mut roots = Vec::new();
                for t in targets {
                    check_cancel(&cancel)?;
                    if key::parent_prefix(&t.key) == dest_prefix {
                        result.skip(&t.key, "移動先が同じフォルダです");
                        continue;
                    }
                    let base =
                        key::move_destination(&t.key, key::parent_prefix(&t.key), &dest_prefix);
                    let new_root = match decisions.get(&base) {
                        ConflictDecision::Skip => {
                            if remote_conflict(&ctx, &base, t.is_folder).await?.is_some() {
                                result.skip(&t.key, "同じ名前の項目があるためスキップしました");
                                continue;
                            }
                            base
                        }
                        ConflictDecision::KeepBoth => {
                            unique_destination(&ctx, &base, t.is_folder).await?
                        }
                        ConflictDecision::Replace => base,
                    };
                    roots.push((t, new_root));
                }
                let moved = relocate(&core, &ctx, &cid, roots, &progress, &cancel).await?;
                result.merge(moved);
                Ok(result)
            },
        ))
    }

    /// 名前の変更。「同じ親フォルダ＋新しい名前」への移動として扱う（04 §7.3）。
    pub async fn rename(
        &self,
        connection_id: &str,
        target: Target,
        new_name: &str,
        sink: Arc<dyn ProgressSink<BatchEvent>>,
    ) -> CoreResult<JobId> {
        key::validate_key(&target.key)?;
        key::validate_name(new_name)?;
        let parent = key::parent_prefix(&target.key);
        let new_root = format!(
            "{parent}{}{}",
            key::nfc(new_name.trim()),
            if target.is_folder { "/" } else { "" }
        );
        key::validate_key(&new_root)?;
        let ctx = self.core.ctx(connection_id).await?;
        if new_root != target.key
            && remote_conflict(&ctx, &new_root, target.is_folder)
                .await?
                .is_some()
        {
            return Err(CoreError::new(ErrorCode::AlreadyExists));
        }
        let core = self.core.clone();
        let cid = connection_id.to_string();
        Ok(spawn_batch(
            self.core,
            JobKind::Move,
            connection_id,
            sink,
            move |progress, cancel| async move {
                if new_root == target.key {
                    return Ok(BatchResult::default());
                }
                relocate(
                    &core,
                    &ctx,
                    &cid,
                    vec![(target, new_root)],
                    &progress,
                    &cancel,
                )
                .await
            },
        ))
    }

    /// ストレージクラスの変更（04 §8.2）。同じキーへのコピーで新しいクラスのオブジェクトを作る。
    pub async fn change_storage_class(
        &self,
        connection_id: &str,
        targets: Vec<Target>,
        storage_class: StorageClass,
        sink: Arc<dyn ProgressSink<BatchEvent>>,
    ) -> CoreResult<JobId> {
        if storage_class == StorageClass::Other {
            return Err(CoreError::internal(
                "OTHER is not a selectable storage class",
            ));
        }
        for t in &targets {
            key::validate_key(&t.key)?;
        }
        let ctx = self.core.ctx(connection_id).await?;
        let core = self.core.clone();
        let cid = connection_id.to_string();
        Ok(spawn_batch(
            self.core,
            JobKind::StorageClass,
            connection_id,
            sink,
            move |progress, cancel| async move {
                let mut objects: Vec<Listed> = Vec::new();
                for t in &targets {
                    check_cancel(&cancel)?;
                    if t.is_folder {
                        let (items, _) = list_recursive(&ctx, &t.key, usize::MAX).await?;
                        // フォルダマーカーは除外する
                        objects.extend(items.into_iter().filter(|o| !key::is_folder_key(&o.key)));
                    } else {
                        objects.push(head(&ctx, &t.key, None).await?.to_listed(&t.key));
                    }
                }
                progress.set_total(objects.len() as u64);
                let outcomes: Vec<(Listed, CoreResult<Option<&'static str>>)> =
                    stream::iter(objects)
                        .map(|o| {
                            let ctx = ctx.clone();
                            let cancel = cancel.clone();
                            let progress = progress.clone();
                            async move {
                                let r = change_one(&ctx, &o, storage_class, &cancel).await;
                                progress.advance(1);
                                (o, r)
                            }
                        })
                        .buffer_unordered(BATCH_CONCURRENCY)
                        .collect()
                        .await;
                let mut result = BatchResult::default();
                let mut changed = Vec::new();
                for (o, r) in outcomes {
                    match r {
                        Ok(None) => {
                            result.succeeded += 1;
                            changed.push(IndexedObject {
                                storage_class,
                                ..o.to_indexed()
                            });
                        }
                        Ok(Some(reason)) => result.skip(&o.key, reason),
                        Err(e) if e.is_canceled() => result.skip(&o.key, "キャンセルしました"),
                        Err(e) => result.fail(&o.key, e),
                    }
                }
                search::upsert(&core.0.db, &cid, changed).await?;
                Ok(result)
            },
        ))
    }
}

/// 変更した場合は `None`、スキップした場合は理由を返す。
async fn change_one(
    ctx: &ConnCtx,
    listed: &Listed,
    class: StorageClass,
    cancel: &CancellationToken,
) -> CoreResult<Option<&'static str>> {
    check_cancel(cancel)?;
    if listed.storage_class == class {
        return Ok(Some("すでに同じストレージクラスです"));
    }
    let source = head(ctx, &listed.key, None).await?;
    if source.restore.needs_restore() {
        return Ok(Some("取り出しが必要なため変更できません"));
    }
    if source.storage_class == class {
        return Ok(Some("すでに同じストレージクラスです"));
    }
    copy::copy(
        ctx,
        CopySpec {
            src_key: &listed.key,
            src_version: None,
            dest_key: &listed.key,
            storage_class: class,
            source: &source,
        },
    )
    .await?;
    Ok(None)
}

/// 「name (n)」形式で、移動先に存在しない名前を探す。
async fn unique_destination(ctx: &ConnCtx, base: &str, is_folder: bool) -> CoreResult<String> {
    let parent = key::parent_prefix(base);
    let name = key::base_name(base);
    for n in 0.. {
        let candidate_name = if n == 0 {
            name.to_string()
        } else {
            key::numbered_name(name, n)
        };
        let candidate = format!(
            "{parent}{candidate_name}{}",
            if is_folder { "/" } else { "" }
        );
        if remote_conflict(ctx, &candidate, is_folder).await?.is_none() {
            return Ok(candidate);
        }
    }
    unreachable!()
}

/// コピー → 確認 → 削除 を 1 オブジェクトずつ行う（04 §7.3）。
async fn relocate(
    core: &Core,
    ctx: &Arc<ConnCtx>,
    connection_id: &str,
    roots: Vec<(Target, String)>,
    progress: &Arc<BatchProgressEmitter>,
    cancel: &CancellationToken,
) -> CoreResult<BatchResult> {
    let mut plan: Vec<(Listed, String)> = Vec::new();
    for (root, new_root) in roots {
        check_cancel(cancel)?;
        let objects: Vec<Listed> = if root.is_folder {
            list_recursive(ctx, &root.key, usize::MAX).await?.0
        } else {
            vec![head(ctx, &root.key, None).await?.to_listed(&root.key)]
        };
        for o in objects {
            if let Some(rest) = o.key.strip_prefix(root.key.as_str()) {
                let dest = format!("{new_root}{rest}");
                plan.push((o, dest));
            }
        }
    }
    progress.set_total(plan.len() as u64);

    let outcomes: Vec<(Listed, String, CoreResult<Option<&'static str>>)> = stream::iter(plan)
        .map(|(o, dest)| {
            let ctx = ctx.clone();
            let cancel = cancel.clone();
            let progress = progress.clone();
            async move {
                let r = relocate_one(&ctx, &o, &dest, &cancel).await;
                progress.advance(1);
                (o, dest, r)
            }
        })
        .buffer_unordered(BATCH_CONCURRENCY)
        .collect()
        .await;

    let mut result = BatchResult::default();
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for (o, dest, r) in outcomes {
        match r {
            Ok(None) => {
                result.succeeded += 1;
                removed.push(o.key.clone());
                added.push(IndexedObject {
                    key: dest,
                    ..o.to_indexed()
                });
            }
            Ok(Some(reason)) => result.skip(&o.key, reason),
            Err(e) if e.is_canceled() => result.skip(&o.key, "キャンセルしました"),
            Err(e) => result.fail(&o.key, e),
        }
    }
    search::remove(&core.0.db, connection_id, removed).await?;
    search::upsert(&core.0.db, connection_id, added).await?;
    Ok(result)
}

async fn relocate_one(
    ctx: &ConnCtx,
    listed: &Listed,
    dest: &str,
    cancel: &CancellationToken,
) -> CoreResult<Option<&'static str>> {
    check_cancel(cancel)?;
    key::validate_key(dest)?;
    if listed.restore.needs_restore() {
        return Ok(Some("取り出されていないアーカイブのため移動できません"));
    }
    let source = head(ctx, &listed.key, None).await?;
    if source.restore.needs_restore() {
        return Ok(Some("取り出されていないアーカイブのため移動できません"));
    }
    copy::copy(
        ctx,
        CopySpec {
            src_key: &listed.key,
            src_version: None,
            dest_key: dest,
            storage_class: source.storage_class,
            source: &source,
        },
    )
    .await?;
    let copied = head(ctx, dest, None).await?;
    if copied.size != source.size {
        return Err(CoreError::internal("copied object size mismatch"));
    }
    ctx.clients
        .s3_bulk
        .delete_object()
        .bucket(&ctx.bucket)
        .key(&listed.key)
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Op("s3:DeleteObject")))?;
    Ok(None)
}

async fn delete_job(
    core: &Core,
    ctx: &Arc<ConnCtx>,
    connection_id: &str,
    targets: Vec<Target>,
    all_versions: bool,
    progress: &Arc<BatchProgressEmitter>,
    cancel: &CancellationToken,
) -> CoreResult<BatchResult> {
    let use_versions = all_versions && ctx.versioning().await.has_history();
    let mut idents: Vec<(String, Option<String>)> = Vec::new();
    for t in &targets {
        check_cancel(cancel)?;
        if use_versions {
            let exact = (!t.is_folder).then_some(t.key.as_str());
            idents.extend(crate::versions::list_all_versions(ctx, &t.key, exact).await?);
        } else if t.is_folder {
            idents.extend(
                list_recursive(ctx, &t.key, usize::MAX)
                    .await?
                    .0
                    .into_iter()
                    .map(|o| (o.key, None)),
            );
        } else {
            idents.push((t.key.clone(), t.version_id.clone()));
        }
    }
    progress.set_total(idents.len() as u64);

    let chunks: Vec<Vec<VersionedKey>> = idents.chunks(1000).map(<[_]>::to_vec).collect();
    let outcomes: Vec<(Vec<VersionedKey>, CoreResult<Vec<KeyError>>)> = stream::iter(chunks)
        .map(|chunk| {
            let ctx = ctx.clone();
            let cancel = cancel.clone();
            let progress = progress.clone();
            async move {
                let r = delete_chunk(&ctx, &chunk, &cancel).await;
                progress.advance(chunk.len() as u64);
                (chunk, r)
            }
        })
        .buffer_unordered(DELETE_CONCURRENCY)
        .collect()
        .await;

    let (result, removed) = tally_deletions(outcomes);
    search::remove(&core.0.db, connection_id, removed).await?;
    Ok(result)
}

/// `DeleteObjects` の結果をキーごとにまとめる。全バージョンの削除では 1 つのキーに複数の識別子
/// （バージョン・削除マーカー）があるため、件数はバージョンではなくキーで数える（DLG-02 の「{n} 項目」）。
/// すべての識別子を削除できたキーを成功とし、インデックスから取り除くキーとして返す。
fn tally_deletions(
    outcomes: Vec<(Vec<VersionedKey>, CoreResult<Vec<KeyError>>)>,
) -> (BatchResult, Vec<String>) {
    let mut keys = BTreeSet::new();
    let mut failed: std::collections::BTreeMap<String, CoreError> = Default::default();
    let mut canceled = BTreeSet::new();
    for (chunk, r) in outcomes {
        keys.extend(chunk.iter().map(|(k, _)| k.clone()));
        match r {
            Ok(errors) => {
                for (k, e) in errors {
                    failed.entry(k).or_insert(e);
                }
            }
            Err(e) if e.is_canceled() => canceled.extend(chunk.into_iter().map(|(k, _)| k)),
            Err(e) => {
                for (k, _) in chunk {
                    failed.entry(k).or_insert_with(|| e.clone());
                }
            }
        }
    }
    let mut result = BatchResult::default();
    let mut removed = Vec::new();
    for k in keys {
        if let Some(e) = failed.remove(&k) {
            result.fail(k, e);
        } else if canceled.contains(&k) {
            result.skip(k, "キャンセルしました");
        } else {
            result.succeeded += 1;
            removed.push(k);
        }
    }
    (result, removed)
}

/// `DeleteObjects`（1 リクエスト 1,000 件、Quiet）。失敗した項目を返す。
async fn delete_chunk(
    ctx: &ConnCtx,
    chunk: &[(String, Option<String>)],
    cancel: &CancellationToken,
) -> CoreResult<Vec<(String, CoreError)>> {
    check_cancel(cancel)?;
    let objects = chunk
        .iter()
        .map(|(k, v)| {
            ObjectIdentifier::builder()
                .key(k)
                .set_version_id(v.clone())
                .build()
                .map_err(CoreError::internal)
        })
        .collect::<CoreResult<Vec<_>>>()?;
    let action = if chunk.iter().any(|(_, v)| v.is_some()) {
        "s3:DeleteObjectVersion"
    } else {
        "s3:DeleteObject"
    };
    let out = ctx
        .clients
        .s3_bulk
        .delete_objects()
        .bucket(&ctx.bucket)
        .delete(
            Delete::builder()
                .set_objects(Some(objects))
                .quiet(true)
                .build()
                .map_err(CoreError::internal)?,
        )
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Op(action)))?;
    Ok(out
        .errors()
        .iter()
        .map(|e| {
            let err = error_map::classify_parts(
                ErrorParts {
                    code: e.code().map(str::to_string),
                    message: e.message().map(str::to_string),
                    ..Default::default()
                },
                Ctx::Op(action),
            );
            (e.key().unwrap_or_default().to_string(), err)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::MemorySink;

    #[test]
    fn counts_deletions_by_key_not_by_version() {
        let v = |k: &str, id: &str| (k.to_string(), Some(id.to_string()));
        let outcomes = vec![
            (
                vec![v("a.txt", "1"), v("a.txt", "2"), v("b.txt", "1")],
                Ok(vec![(
                    "b.txt".to_string(),
                    CoreError::new(ErrorCode::ObjectLocked),
                )]),
            ),
            (vec![v("a.txt", "3"), v("c.txt", "1")], Ok(vec![])),
            (vec![v("d.txt", "1")], Err(CoreError::canceled())),
        ];
        let (result, removed) = tally_deletions(outcomes);
        // a.txt の 3 つのバージョンは 1 項目として数える
        assert_eq!(result.succeeded, 2);
        assert_eq!(removed, vec!["a.txt".to_string(), "c.txt".to_string()]);
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].key, "b.txt");
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].key, "d.txt");
    }

    #[test]
    fn maps_folder_contents_to_the_new_root() {
        let keys = vec![
            "a/b/".to_string(),
            "a/b/c.txt".to_string(),
            "a/bx.txt".to_string(),
        ];
        assert_eq!(
            relocation_plan(&keys, "a/b/", "x/b/"),
            vec![
                ("a/b/".to_string(), "x/b/".to_string()),
                ("a/b/c.txt".to_string(), "x/b/c.txt".to_string())
            ]
        );
    }

    #[test]
    fn throttles_progress_but_always_sends_the_last_one() {
        let sink = Arc::new(MemorySink::<BatchEvent>::default());
        let p = BatchProgressEmitter::new("j".into(), sink.clone());
        p.set_total(100);
        for _ in 0..100 {
            p.advance(1);
        }
        let events = sink.events();
        assert!(events.len() < 10, "{} events", events.len());
        match events.last().unwrap() {
            BatchEvent::Progress(p) => assert_eq!((p.done, p.total), (100, Some(100))),
            _ => panic!(),
        }
    }

    #[tokio::test]
    async fn reports_keys_that_delete_objects_could_not_delete() {
        use aws_sdk_s3::operation::delete_objects::DeleteObjectsOutput;
        use aws_sdk_s3::types::{DeletedObject, Error as S3Error};
        use aws_smithy_mocks::{RuleMode, mock, mock_client};

        // DeleteObjects は一部の項目だけ失敗しても 200 を返し、失敗を Errors に入れる（04 §6.3）
        let rule = mock!(aws_sdk_s3::Client::delete_objects).then_output(|| {
            DeleteObjectsOutput::builder()
                .deleted(DeletedObject::builder().key("a.txt").build())
                .errors(
                    S3Error::builder()
                        .key("b.txt")
                        .code("AccessDenied")
                        .message("Access Denied")
                        .build(),
                )
                .build()
        });
        let s3 = mock_client!(aws_sdk_s3, RuleMode::MatchAny, [&rule]);
        let (core, _dir) = Core::for_tests(None).unwrap();
        core.with_test_connection(crate::connections::ConnCtx::for_tests("k1", "b", s3))
            .await;
        let sink = Arc::new(MemorySink::<BatchEvent>::default());
        core.objects()
            .delete(
                "k1",
                vec![Target::file("a.txt"), Target::file("b.txt")],
                false,
                sink.clone(),
            )
            .await
            .unwrap();
        let result = loop {
            if let Some(BatchEvent::Finished(f)) = sink.events().into_iter().last() {
                break f.result;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        assert_eq!(result.succeeded, 1);
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].key, "b.txt");
        assert_eq!(result.failed[0].error.code, ErrorCode::AccessDenied);
        // 必要な権限を示す（04 §6.3）
        assert!(result.failed[0].error.message.contains("s3:DeleteObject"));
    }

    #[tokio::test]
    async fn rejects_moving_a_folder_into_itself() {
        let (core, _dir) = Core::for_tests(None).unwrap();
        let err = core
            .objects()
            .move_objects(
                "c",
                vec![Target::folder("a/")],
                "a/b/".into(),
                Decisions::default(),
                Arc::new(crate::jobs::NullSink),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidName);
    }
}
