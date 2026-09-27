//! オブジェクト・フォルダ操作（04 §3、§6〜§9）。

mod batch;
mod copy;
mod restore;

use std::collections::{BTreeMap, BTreeSet};

use aws_sdk_s3::types::{ChecksumMode, EncodingType, OptionalObjectAttributes};

use crate::Core;
use crate::aws::error_map::{self, Ctx};
use crate::connections::{ConnCtx, encryption_label};
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::{
    CreatedInfo, CreatedSource, Entry, FolderSummary, ListOptions, ListPage, ObjectDetail,
    RemoteConflict, RestoreState, StorageClass, Target,
};
use crate::search::{self, IndexedObject};
use crate::util::{key, time};

pub use batch::relocation_plan;
pub use copy::MULTIPART_COPY_THRESHOLD;
pub use restore::parse_restore_header;

/// フォルダの集計で列挙する上限（04 §9.3）。
pub const SUMMARY_LIMIT: usize = 100_000;
/// 削除済みの項目の表示で読む `ListObjectVersions` のページ数の上限。
const DELETED_SCAN_PAGES: usize = 20;

/// 列挙したオブジェクト。
#[derive(Debug, Clone)]
pub struct Listed {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
    pub etag: String,
    pub storage_class: StorageClass,
    pub restore: RestoreState,
}

impl Listed {
    pub fn to_indexed(&self) -> IndexedObject {
        IndexedObject {
            key: self.key.clone(),
            size: self.size,
            last_modified: self.last_modified.clone(),
            etag: Some(self.etag.clone()),
            storage_class: self.storage_class,
        }
    }

    fn to_entry(&self) -> Entry {
        Entry::File {
            key: self.key.clone(),
            name: key::nfc(key::base_name(&self.key)),
            size: self.size,
            last_modified: self.last_modified.clone(),
            etag: self.etag.clone(),
            storage_class: self.storage_class,
            restore: self.restore.clone(),
            deleted: false,
        }
    }
}

pub(crate) fn trim_etag(etag: Option<&str>) -> String {
    etag.unwrap_or_default().trim_matches('"').to_string()
}

/// 一覧の `RestoreStatus` から取り出し状態を求める（04 §8.4）。
fn restore_from_list(
    class: StorageClass,
    status: Option<&aws_sdk_s3::types::RestoreStatus>,
) -> RestoreState {
    if !class.is_archive() {
        return RestoreState::NotArchived;
    }
    match status {
        Some(s) if s.is_restore_in_progress() == Some(true) => RestoreState::InProgress,
        Some(s) => match s.restore_expiry_date() {
            Some(expiry) => RestoreState::Restored {
                expiry: time::aws_to_rfc3339(expiry),
            },
            None => RestoreState::Archived,
        },
        None => RestoreState::Archived,
    }
}

fn listed_from(o: &aws_sdk_s3::types::Object) -> Option<Listed> {
    let key = key::url_decode_key(o.key()?);
    let class = StorageClass::from_s3(o.storage_class().map(|c| c.as_str()));
    Some(Listed {
        size: o.size().unwrap_or(0).max(0) as u64,
        last_modified: o
            .last_modified()
            .map(time::aws_to_rfc3339)
            .unwrap_or_default(),
        etag: trim_etag(o.e_tag()),
        restore: restore_from_list(class, o.restore_status()),
        storage_class: class,
        key,
    })
}

pub struct ObjectService<'a> {
    core: &'a Core,
}

impl Core {
    pub fn objects(&self) -> ObjectService<'_> {
        ObjectService { core: self }
    }
}

/// プレフィックス配下を区切り文字なしで列挙する（`limit` 件まで）。戻り値の真偽は上限に達したか。
pub(crate) async fn list_recursive(
    ctx: &ConnCtx,
    prefix: &str,
    limit: usize,
) -> CoreResult<(Vec<Listed>, bool)> {
    let mut token = None;
    let mut out = Vec::new();
    loop {
        let page = ctx
            .clients
            .s3
            .list_objects_v2()
            .bucket(&ctx.bucket)
            .prefix(prefix)
            .max_keys(1000)
            .encoding_type(EncodingType::Url)
            .optional_object_attributes(OptionalObjectAttributes::RestoreStatus)
            .set_continuation_token(token)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucket")))?;
        out.extend(page.contents().iter().filter_map(listed_from));
        if out.len() >= limit {
            out.truncate(limit);
            return Ok((out, true));
        }
        token = page.next_continuation_token().map(str::to_string);
        if token.is_none() {
            return Ok((out, false));
        }
    }
}

/// 1 つのキーのメタデータ（アーカイブの状態を含む）。
pub(crate) async fn head(
    ctx: &ConnCtx,
    key: &str,
    version_id: Option<&str>,
) -> CoreResult<HeadInfo> {
    let out = ctx
        .clients
        .s3
        .head_object()
        .bucket(&ctx.bucket)
        .key(key)
        .set_version_id(version_id.map(str::to_string))
        .checksum_mode(ChecksumMode::Enabled)
        .send()
        .await
        .map_err(|e| {
            error_map::classify(
                &e,
                Ctx::Op(if version_id.is_some() {
                    "s3:GetObjectVersion"
                } else {
                    "s3:GetObject"
                }),
            )
        })?;
    let class = StorageClass::from_s3(out.storage_class().map(|c| c.as_str()));
    let archived_tier = out
        .archive_status()
        .is_some_and(|s| matches!(s.as_str(), "ARCHIVE_ACCESS" | "DEEP_ARCHIVE_ACCESS"));
    let restore = parse_restore_header(out.restore(), class.is_archive() || archived_tier);
    let mut checksums = BTreeMap::new();
    for (name, value) in [
        ("CRC32", out.checksum_crc32()),
        ("CRC32C", out.checksum_crc32_c()),
        ("CRC64NVME", out.checksum_crc64_nvme()),
        ("SHA1", out.checksum_sha1()),
        ("SHA256", out.checksum_sha256()),
    ] {
        if let Some(v) = value {
            checksums.insert(name.to_string(), v.to_string());
        }
    }
    Ok(HeadInfo {
        size: out.content_length().unwrap_or(0).max(0) as u64,
        content_type: out
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string(),
        last_modified: out
            .last_modified()
            .map(time::aws_to_rfc3339)
            .unwrap_or_default(),
        etag: trim_etag(out.e_tag()),
        storage_class: class,
        version_id: out
            .version_id()
            .filter(|v| *v != "null")
            .map(str::to_string),
        sse: out.server_side_encryption().map(|s| s.as_str().to_string()),
        kms_key_id: out.ssekms_key_id().map(str::to_string),
        metadata: out
            .metadata()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default(),
        restore,
        checksums,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct HeadInfo {
    pub size: u64,
    pub content_type: String,
    pub last_modified: String,
    pub etag: String,
    pub storage_class: StorageClass,
    pub version_id: Option<String>,
    pub sse: Option<String>,
    pub kms_key_id: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub restore: RestoreState,
    pub checksums: BTreeMap<String, String>,
}

impl HeadInfo {
    pub fn to_listed(&self, key: &str) -> Listed {
        Listed {
            key: key.to_string(),
            size: self.size,
            last_modified: self.last_modified.clone(),
            etag: self.etag.clone(),
            storage_class: self.storage_class,
            restore: self.restore.clone(),
        }
    }
}

impl ObjectService<'_> {
    /// フォルダの一覧の 1 ページ（04 §3.1）。
    pub async fn list_page(
        &self,
        connection_id: &str,
        prefix: &str,
        token: Option<String>,
        opts: ListOptions,
    ) -> CoreResult<ListPage> {
        key::validate_prefix(prefix)?;
        let ctx = self.core.ctx(connection_id).await?;
        let first_page = token.is_none();
        let out = ctx
            .clients
            .s3
            .list_objects_v2()
            .bucket(&ctx.bucket)
            .prefix(prefix)
            .delimiter("/")
            .max_keys(1000)
            .encoding_type(EncodingType::Url)
            .optional_object_attributes(OptionalObjectAttributes::RestoreStatus)
            .set_continuation_token(token)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucket")))?;

        let folder_dates =
            search::folder_marker_dates(&self.core.0.db, connection_id, prefix).await?;
        let mut entries: Vec<Entry> = out
            .common_prefixes()
            .iter()
            .filter_map(|p| p.prefix())
            .map(|p| {
                let key = key::url_decode_key(p);
                Entry::Folder {
                    name: key::nfc(key::base_name(&key)),
                    last_modified: folder_dates.get(&key).cloned(),
                    key,
                    deleted: false,
                }
            })
            .collect();
        entries.extend(
            out.contents()
                .iter()
                .filter_map(listed_from)
                // 現在のフォルダのフォルダマーカーは除く
                .filter(|l| l.key != prefix)
                .map(|l| l.to_entry()),
        );
        if opts.include_deleted && first_page {
            entries.extend(self.deleted_entries(&ctx, prefix, &entries).await?);
        }
        if !opts.show_hidden {
            entries.retain(|e| !key::is_hidden(e.name()));
        }
        Ok(ListPage {
            entries,
            next_token: out.next_continuation_token().map(str::to_string),
        })
    }

    /// 最新が削除マーカーのキーと、現行の項目がなくなったプレフィックス（04 §3.3）。
    async fn deleted_entries(
        &self,
        ctx: &ConnCtx,
        prefix: &str,
        current: &[Entry],
    ) -> CoreResult<Vec<Entry>> {
        let mut key_marker = None;
        let mut version_marker = None;
        let mut latest_markers: BTreeMap<String, String> = BTreeMap::new();
        let mut last_versions: BTreeMap<String, (u64, StorageClass)> = BTreeMap::new();
        let mut version_prefixes = BTreeSet::new();
        for _ in 0..DELETED_SCAN_PAGES {
            let out = ctx
                .clients
                .s3
                .list_object_versions()
                .bucket(&ctx.bucket)
                .prefix(prefix)
                .delimiter("/")
                .encoding_type(EncodingType::Url)
                .set_key_marker(key_marker)
                .set_version_id_marker(version_marker)
                .send()
                .await
                .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucketVersions")))?;
            for m in out.delete_markers() {
                if m.is_latest() == Some(true)
                    && let Some(k) = m.key()
                {
                    latest_markers.insert(
                        key::url_decode_key(k),
                        m.last_modified()
                            .map(time::aws_to_rfc3339)
                            .unwrap_or_default(),
                    );
                }
            }
            for v in out.versions() {
                if let Some(k) = v.key() {
                    // 新しい順に返るため、最初に出たものがそのキーの最新の実体
                    last_versions.entry(key::url_decode_key(k)).or_insert((
                        v.size().unwrap_or(0).max(0) as u64,
                        StorageClass::from_s3(v.storage_class().map(|c| c.as_str())),
                    ));
                }
            }
            for p in out.common_prefixes().iter().filter_map(|p| p.prefix()) {
                version_prefixes.insert(key::url_decode_key(p));
            }
            if out.is_truncated() != Some(true) {
                break;
            }
            key_marker = out.next_key_marker().map(str::to_string);
            version_marker = out.next_version_id_marker().map(str::to_string);
        }

        let mut entries = Vec::new();
        for (k, modified) in latest_markers {
            if k == prefix {
                continue;
            }
            let (size, class) = last_versions
                .get(&k)
                .copied()
                .unwrap_or((0, StorageClass::Standard));
            entries.push(Entry::File {
                name: key::nfc(key::base_name(&k)),
                size,
                last_modified: modified,
                etag: String::new(),
                storage_class: class,
                restore: RestoreState::NotArchived,
                deleted: true,
                key: k,
            });
        }
        let current_keys: BTreeSet<&str> = current.iter().map(Entry::key).collect();
        for p in version_prefixes
            .into_iter()
            .filter(|p| !current_keys.contains(p.as_str()))
            .take(100)
        {
            let out = ctx
                .clients
                .s3
                .list_objects_v2()
                .bucket(&ctx.bucket)
                .prefix(&p)
                .max_keys(1)
                .send()
                .await
                .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucket")))?;
            if out.key_count().unwrap_or(0) == 0 {
                entries.push(Entry::Folder {
                    name: key::nfc(key::base_name(&p)),
                    last_modified: None,
                    key: p,
                    deleted: true,
                });
            }
        }
        Ok(entries)
    }

    /// メタデータ（04 §9）。
    pub async fn head(
        &self,
        connection_id: &str,
        object_key: &str,
        version_id: Option<&str>,
    ) -> CoreResult<ObjectDetail> {
        key::validate_key(object_key)?;
        let ctx = self.core.ctx(connection_id).await?;
        let info = head(&ctx, object_key, version_id).await?;
        let created = if ctx.versioning().await.has_history() {
            match crate::versions::oldest_version_date(&ctx, object_key).await {
                Ok(Some(at)) => Some(CreatedInfo {
                    at,
                    source: CreatedSource::OldestVersion,
                }),
                _ => None,
            }
        } else {
            None
        };
        let created = created
            .or_else(|| {
                info.metadata
                    .get("s3drive-created")
                    .and_then(|v| time::parse_rfc3339(v))
                    .map(|at| CreatedInfo {
                        at: time::to_rfc3339(at),
                        source: CreatedSource::Metadata,
                    })
            })
            .unwrap_or_else(|| CreatedInfo {
                at: info.last_modified.clone(),
                source: CreatedSource::LastModified,
            });
        Ok(ObjectDetail {
            key: object_key.to_string(),
            version_id: info.version_id.clone(),
            size: info.size,
            content_type: info.content_type.clone(),
            last_modified: info.last_modified.clone(),
            created,
            etag: info.etag.clone(),
            storage_class: info.storage_class,
            encryption: encryption_label(
                info.sse.as_deref().unwrap_or(""),
                info.kms_key_id.as_deref(),
            ),
            kms_key_id: info.kms_key_id.clone(),
            checksums: info.checksums.clone(),
            user_metadata: info.metadata.clone(),
            restore: info.restore.clone(),
        })
    }

    /// フォルダマーカーを作る（04 §7.1）。
    pub async fn create_folder(
        &self,
        connection_id: &str,
        prefix: &str,
        name: &str,
    ) -> CoreResult<Entry> {
        key::validate_prefix(prefix)?;
        key::validate_name(name)?;
        let folder_key = format!("{prefix}{}/", key::nfc(name.trim()));
        key::validate_key(&folder_key)?;
        let ctx = self.core.ctx(connection_id).await?;
        let existing = ctx
            .clients
            .s3
            .list_objects_v2()
            .bucket(&ctx.bucket)
            .prefix(&folder_key)
            .max_keys(1)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucket")))?;
        if existing.key_count().unwrap_or(0) > 0 {
            return Err(CoreError::with_message(
                ErrorCode::AlreadyExists,
                "同じ名前のフォルダがあります",
            ));
        }
        let out = ctx
            .clients
            .s3
            .put_object()
            .bucket(&ctx.bucket)
            .key(&folder_key)
            .content_length(0)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:PutObject")))?;
        let now = time::now_rfc3339();
        search::upsert(
            &self.core.0.db,
            connection_id,
            vec![IndexedObject {
                key: folder_key.clone(),
                size: 0,
                last_modified: now.clone(),
                etag: Some(trim_etag(out.e_tag())),
                storage_class: StorageClass::Standard,
            }],
        )
        .await?;
        Ok(Entry::Folder {
            name: key::nfc(name.trim()),
            key: folder_key,
            last_modified: Some(now),
            deleted: false,
        })
    }

    /// フォルダの項目数と合計サイズ（04 §9.3）。インデックスがあればそこから集計する。
    pub async fn folder_summary(
        &self,
        connection_id: &str,
        prefix: &str,
    ) -> CoreResult<FolderSummary> {
        key::validate_prefix(prefix)?;
        let ctx = self.core.ctx(connection_id).await?;
        if let Some(summary) =
            search::folder_summary(&self.core.0.db, connection_id, prefix).await?
        {
            return Ok(summary);
        }
        let (objects, truncated) = list_recursive(&ctx, prefix, SUMMARY_LIMIT).await?;
        Ok(summarize(prefix, &objects, truncated))
    }

    /// 移動ダイアログのツリー用に、直下のフォルダだけを返す。
    pub async fn folder_children(
        &self,
        connection_id: &str,
        prefix: &str,
    ) -> CoreResult<Vec<Entry>> {
        key::validate_prefix(prefix)?;
        let ctx = self.core.ctx(connection_id).await?;
        let mut token = None;
        let mut folders = Vec::new();
        for _ in 0..20 {
            let out = ctx
                .clients
                .s3
                .list_objects_v2()
                .bucket(&ctx.bucket)
                .prefix(prefix)
                .delimiter("/")
                .max_keys(1000)
                .encoding_type(EncodingType::Url)
                .set_continuation_token(token)
                .send()
                .await
                .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucket")))?;
            folders.extend(
                out.common_prefixes()
                    .iter()
                    .filter_map(|p| p.prefix())
                    .map(|p| {
                        let key = key::url_decode_key(p);
                        Entry::Folder {
                            name: key::nfc(key::base_name(&key)),
                            key,
                            last_modified: None,
                            deleted: false,
                        }
                    }),
            );
            token = out.next_continuation_token().map(str::to_string);
            if token.is_none() {
                break;
            }
        }
        Ok(folders)
    }

    /// 移動先の同名の項目（DLG-08）。
    pub async fn find_conflicts(
        &self,
        connection_id: &str,
        targets: &[Target],
        dest_prefix: &str,
    ) -> CoreResult<Vec<RemoteConflict>> {
        key::validate_prefix(dest_prefix)?;
        let ctx = self.core.ctx(connection_id).await?;
        let mut conflicts = Vec::new();
        for t in targets {
            if key::parent_prefix(&t.key) == dest_prefix {
                continue;
            }
            let dest = key::move_destination(&t.key, key::parent_prefix(&t.key), dest_prefix);
            if let Some(c) = remote_conflict(&ctx, &dest, t.is_folder).await? {
                conflicts.push(c);
            }
        }
        Ok(conflicts)
    }
}

/// `dest`（ファイルのキー、またはフォルダのプレフィックス）に既存の項目があるか。
pub(crate) async fn remote_conflict(
    ctx: &ConnCtx,
    dest: &str,
    is_folder: bool,
) -> CoreResult<Option<RemoteConflict>> {
    let out = ctx
        .clients
        .s3
        .list_objects_v2()
        .bucket(&ctx.bucket)
        .prefix(dest)
        .max_keys(if is_folder { 1 } else { 1000 })
        .encoding_type(EncodingType::Url)
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucket")))?;
    let hit = out
        .contents()
        .iter()
        .filter_map(listed_from)
        .find(|l| is_folder || l.key == dest);
    Ok(hit.map(|l| RemoteConflict {
        key: dest.to_string(),
        remote_size: if is_folder { 0 } else { l.size },
        remote_modified: l.last_modified,
    }))
}

/// 以前のバージョンを同じキーにコピーする（バージョンの復元。04 §11.3）。
pub(crate) async fn copy_version(
    ctx: &ConnCtx,
    object_key: &str,
    version_id: &str,
    source: &HeadInfo,
) -> CoreResult<()> {
    copy::copy(
        ctx,
        copy::CopySpec {
            src_key: object_key,
            src_version: Some(version_id),
            dest_key: object_key,
            storage_class: source.storage_class,
            source,
        },
    )
    .await
}

/// 列挙結果からフォルダの直下の件数と合計サイズを求める。
pub fn summarize(prefix: &str, objects: &[Listed], truncated: bool) -> FolderSummary {
    let mut children = BTreeSet::new();
    let mut total = 0u64;
    for o in objects {
        let Some(rest) = o.key.strip_prefix(prefix) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        total += o.size;
        let first = match rest.find('/') {
            Some(i) => &rest[..=i],
            None => rest,
        };
        children.insert(first.to_string());
    }
    FolderSummary {
        item_count: children.len() as u64,
        total_bytes: total,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(k: &str, size: u64) -> Listed {
        Listed {
            key: k.into(),
            size,
            last_modified: String::new(),
            etag: String::new(),
            storage_class: StorageClass::Standard,
            restore: RestoreState::NotArchived,
        }
    }

    #[test]
    fn summarizes_direct_children_and_total_size() {
        let objects = vec![
            listed("p/", 0),
            listed("p/a.txt", 10),
            listed("p/sub/b.txt", 20),
            listed("p/sub/c.txt", 30),
            listed("p/empty/", 0),
        ];
        let s = summarize("p/", &objects, false);
        assert_eq!(s.item_count, 3);
        assert_eq!(s.total_bytes, 60);
    }

    #[test]
    fn derives_restore_state_from_listing() {
        use aws_sdk_s3::types::RestoreStatus;
        assert_eq!(
            restore_from_list(StorageClass::Standard, None),
            RestoreState::NotArchived
        );
        assert_eq!(
            restore_from_list(StorageClass::Glacier, None),
            RestoreState::Archived
        );
        let ongoing = RestoreStatus::builder()
            .is_restore_in_progress(true)
            .build();
        assert_eq!(
            restore_from_list(StorageClass::DeepArchive, Some(&ongoing)),
            RestoreState::InProgress
        );
        let done = RestoreStatus::builder()
            .is_restore_in_progress(false)
            .restore_expiry_date(aws_smithy_types::DateTime::from_secs(1_790_000_000))
            .build();
        assert!(matches!(
            restore_from_list(StorageClass::Glacier, Some(&done)),
            RestoreState::Restored { .. }
        ));
    }
}
