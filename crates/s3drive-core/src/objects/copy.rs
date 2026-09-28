//! オブジェクトのコピー（移動・クラス変更・バージョン復元に使う。D6）。

use aws_sdk_s3::types::{
    CompletedMultipartUpload, CompletedPart, MetadataDirective, ServerSideEncryption,
    TaggingDirective,
};
use futures::{StreamExt, stream};

use super::HeadInfo;
use crate::aws::error_map::{self, Ctx};
use crate::connections::ConnCtx;
use crate::error::{CoreError, CoreResult};
use crate::model::StorageClass;
use crate::util::key;

/// これを超えるオブジェクトは UploadPartCopy で分割してコピーする（5 GB）。
pub const MULTIPART_COPY_THRESHOLD: u64 = 5 * 1024 * 1024 * 1024;
/// マルチパートコピーのパートサイズ。
const COPY_PART_SIZE: u64 = 512 * 1024 * 1024;

/// コピーの指定。
pub(crate) struct CopySpec<'a> {
    pub src_key: &'a str,
    pub src_version: Option<&'a str>,
    pub dest_key: &'a str,
    /// コピー先のクラス。`CopyObject` は指定しないと STANDARD になるため、必ず指定する（04 §7.3）。
    pub storage_class: StorageClass,
    /// 元のメタデータ（サイズ・暗号化・Content-Type など）。
    pub source: &'a HeadInfo,
}

pub(crate) async fn copy(ctx: &ConnCtx, spec: CopySpec<'_>) -> CoreResult<()> {
    if spec.source.size > MULTIPART_COPY_THRESHOLD {
        return multipart_copy(ctx, spec).await;
    }
    let s3 = &ctx.clients.s3_bulk;
    let mut req = s3
        .copy_object()
        .bucket(&ctx.bucket)
        .key(spec.dest_key)
        .copy_source(key::copy_source(
            &ctx.bucket,
            spec.src_key,
            spec.src_version,
        ))
        .metadata_directive(MetadataDirective::Copy)
        .tagging_directive(TaggingDirective::Copy);
    if spec.storage_class != StorageClass::Other {
        req = req.storage_class(spec.storage_class.to_sdk());
    }
    if let Some(kms) = kms_key(spec.source) {
        // 既定と異なる KMS キーで暗号化されている場合に備え、同じキー ID を指定する
        req = req
            .server_side_encryption(ServerSideEncryption::AwsKms)
            .ssekms_key_id(kms);
    }
    req.send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Op("s3:PutObject")))?;
    Ok(())
}

fn kms_key(info: &HeadInfo) -> Option<String> {
    (info.sse.as_deref() == Some("aws:kms"))
        .then(|| info.kms_key_id.clone())
        .flatten()
}

/// 5 GB を超えるオブジェクトのコピー。メタデータとタグは自動で引き継がれないため明示する。
async fn multipart_copy(ctx: &ConnCtx, spec: CopySpec<'_>) -> CoreResult<()> {
    let s3 = &ctx.clients.s3_bulk;
    let tagging = s3
        .get_object_tagging()
        .bucket(&ctx.bucket)
        .key(spec.src_key)
        .set_version_id(spec.src_version.map(str::to_string))
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Op("s3:GetObjectTagging")))?;
    let tag_string = tagging
        .tag_set()
        .iter()
        .map(|t| {
            url::form_urlencoded::Serializer::new(String::new())
                .append_pair(t.key(), t.value())
                .finish()
        })
        .collect::<Vec<_>>()
        .join("&");

    let mut create = s3
        .create_multipart_upload()
        .bucket(&ctx.bucket)
        .key(spec.dest_key)
        .content_type(&spec.source.content_type)
        .set_metadata(Some(spec.source.metadata.clone().into_iter().collect()));
    if spec.storage_class != StorageClass::Other {
        create = create.storage_class(spec.storage_class.to_sdk());
    }
    if !tag_string.is_empty() {
        create = create.tagging(tag_string);
    }
    if let Some(kms) = kms_key(spec.source) {
        create = create
            .server_side_encryption(ServerSideEncryption::AwsKms)
            .ssekms_key_id(kms);
    }
    let upload = create
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Op("s3:PutObject")))?;
    let upload_id = upload
        .upload_id()
        .ok_or_else(|| CoreError::internal("no upload id"))?
        .to_string();

    let size = spec.source.size;
    let part_size = COPY_PART_SIZE.max(size.div_ceil(10_000));
    let parts = size.div_ceil(part_size);
    let source = key::copy_source(&ctx.bucket, spec.src_key, spec.src_version);
    let results: Vec<CoreResult<CompletedPart>> = stream::iter(0..parts)
        .map(|i| {
            let (s3, bucket, dest, upload_id, source) = (
                s3.clone(),
                ctx.bucket.clone(),
                spec.dest_key.to_string(),
                upload_id.clone(),
                source.clone(),
            );
            async move {
                let start = i * part_size;
                let end = (start + part_size).min(size) - 1;
                let out = s3
                    .upload_part_copy()
                    .bucket(bucket)
                    .key(dest)
                    .upload_id(upload_id)
                    .part_number((i + 1) as i32)
                    .copy_source(source)
                    .copy_source_range(format!("bytes={start}-{end}"))
                    .send()
                    .await
                    .map_err(|e| error_map::classify(&e, Ctx::Op("s3:PutObject")))?;
                Ok(CompletedPart::builder()
                    .part_number((i + 1) as i32)
                    .set_e_tag(
                        out.copy_part_result()
                            .and_then(|r| r.e_tag())
                            .map(str::to_string),
                    )
                    .build())
            }
        })
        .buffered(4)
        .collect()
        .await;

    let completed: CoreResult<Vec<CompletedPart>> = results.into_iter().collect();
    let finished = match completed {
        Ok(parts) => s3
            .complete_multipart_upload()
            .bucket(&ctx.bucket)
            .key(spec.dest_key)
            .upload_id(&upload_id)
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(parts))
                    .build(),
            )
            .send()
            .await
            .map(|_| ())
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:PutObject"))),
        Err(e) => Err(e),
    };
    if finished.is_err() {
        let _ = s3
            .abort_multipart_upload()
            .bucket(&ctx.bucket)
            .key(spec.dest_key)
            .upload_id(&upload_id)
            .send()
            .await;
    }
    finished
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use aws_sdk_s3::operation::complete_multipart_upload::CompleteMultipartUploadOutput;
    use aws_sdk_s3::operation::create_multipart_upload::CreateMultipartUploadOutput;
    use aws_sdk_s3::operation::get_object_tagging::GetObjectTaggingOutput;
    use aws_sdk_s3::operation::upload_part_copy::UploadPartCopyOutput;
    use aws_sdk_s3::types::{CopyPartResult, Tag};
    use aws_smithy_mocks::{RuleMode, mock, mock_client};

    use super::*;
    use crate::model::RestoreState;

    fn source(size: u64) -> HeadInfo {
        HeadInfo {
            size,
            content_type: "video/quicktime".into(),
            last_modified: "2026-09-27T00:00:00Z".into(),
            etag: "e".into(),
            storage_class: StorageClass::StandardIa,
            version_id: None,
            sse: Some("aws:kms".into()),
            kms_key_id: Some("key-1".into()),
            metadata: BTreeMap::from([(
                "s3drive-mtime".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
            )]),
            restore: RestoreState::NotArchived,
            checksums: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn copies_objects_over_5_gb_in_parts_with_metadata_and_tags() {
        // 5 GB を超えると CopyObject が使えないため、UploadPartCopy で分割してコピーする（04 §7.3、D6）
        let tagging = mock!(aws_sdk_s3::Client::get_object_tagging).then_output(|| {
            GetObjectTaggingOutput::builder()
                .tag_set(Tag::builder().key("team").value("media").build().unwrap())
                .build()
                .unwrap()
        });
        // マルチパートコピーではメタデータ・タグ・クラス・暗号化が引き継がれないため、明示する
        let create = mock!(aws_sdk_s3::Client::create_multipart_upload)
            .match_requests(|r| {
                r.key() == Some("dest/big.mov")
                    && r.storage_class() == Some(&StorageClass::StandardIa.to_sdk())
                    && r.content_type() == Some("video/quicktime")
                    && r.metadata().and_then(|m| m.get("s3drive-mtime")).is_some()
                    && r.tagging() == Some("team=media")
                    && r.ssekms_key_id() == Some("key-1")
            })
            .then_output(|| {
                CreateMultipartUploadOutput::builder()
                    .upload_id("u1")
                    .build()
            });
        let ranges = Arc::new(Mutex::new(Vec::new()));
        let seen = ranges.clone();
        let part = mock!(aws_sdk_s3::Client::upload_part_copy).then_compute_output(move |r| {
            seen.lock().unwrap().push((
                r.part_number().unwrap(),
                r.copy_source_range().unwrap().to_string(),
            ));
            UploadPartCopyOutput::builder()
                .copy_part_result(
                    CopyPartResult::builder()
                        .e_tag(format!("\"p{}\"", r.part_number().unwrap()))
                        .build(),
                )
                .build()
        });
        let complete = mock!(aws_sdk_s3::Client::complete_multipart_upload)
            .match_requests(|r| {
                r.upload_id() == Some("u1")
                    && r.multipart_upload().is_some_and(|m| m.parts().len() == 12)
            })
            .then_output(|| CompleteMultipartUploadOutput::builder().build());
        let s3 = mock_client!(
            aws_sdk_s3,
            RuleMode::MatchAny,
            [&tagging, &create, &part, &complete]
        );
        let ctx = ConnCtx::for_tests("k1", "b", s3);
        let size = 6 * 1024 * 1024 * 1024u64;
        let info = source(size);
        copy(
            &ctx,
            CopySpec {
                src_key: "src/big.mov",
                src_version: None,
                dest_key: "dest/big.mov",
                storage_class: info.storage_class,
                source: &info,
            },
        )
        .await
        .unwrap();
        assert_eq!(complete.num_calls(), 1);
        let mut ranges = ranges.lock().unwrap().clone();
        ranges.sort();
        assert_eq!(ranges.len(), 12);
        assert_eq!(ranges[0], (1, format!("bytes=0-{}", COPY_PART_SIZE - 1)));
        assert_eq!(
            ranges[11].1,
            format!("bytes={}-{}", 11 * COPY_PART_SIZE, size - 1)
        );
    }

    #[tokio::test]
    async fn aborts_the_multipart_copy_when_a_part_fails() {
        let tagging = mock!(aws_sdk_s3::Client::get_object_tagging).then_output(|| {
            GetObjectTaggingOutput::builder()
                .set_tag_set(Some(vec![]))
                .build()
                .unwrap()
        });
        let create = mock!(aws_sdk_s3::Client::create_multipart_upload).then_output(|| {
            CreateMultipartUploadOutput::builder()
                .upload_id("u1")
                .build()
        });
        let part = mock!(aws_sdk_s3::Client::upload_part_copy).then_http_response(|| {
            aws_smithy_runtime_api::client::orchestrator::HttpResponse::new(
                aws_smithy_runtime_api::http::StatusCode::try_from(403).unwrap(),
                aws_smithy_types::body::SdkBody::from("<Error><Code>AccessDenied</Code></Error>"),
            )
        });
        let abort = mock!(aws_sdk_s3::Client::abort_multipart_upload)
            .match_requests(|r| r.upload_id() == Some("u1"))
            .then_output(|| {
                aws_sdk_s3::operation::abort_multipart_upload::AbortMultipartUploadOutput::builder()
                    .build()
            });
        let s3 = mock_client!(
            aws_sdk_s3,
            RuleMode::MatchAny,
            [&tagging, &create, &part, &abort]
        );
        let ctx = ConnCtx::for_tests("k1", "b", s3);
        let info = source(6 * 1024 * 1024 * 1024);
        let err = copy(
            &ctx,
            CopySpec {
                src_key: "src/big.mov",
                src_version: None,
                dest_key: "dest/big.mov",
                storage_class: info.storage_class,
                source: &info,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, crate::ErrorCode::AccessDenied);
        assert_eq!(abort.num_calls(), 1);
    }
}
