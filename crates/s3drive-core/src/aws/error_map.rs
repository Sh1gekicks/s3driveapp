//! AWS SDK のエラーを [`CoreError`] に変換する（01 §7.1、05 §5）。

use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use aws_smithy_runtime_api::client::orchestrator::HttpResponse;

use crate::error::{CoreError, ErrorCode};

/// エラーが起きた文脈。`AccessDenied` などの意味が文脈で変わる（05 §5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ctx {
    /// 一般の操作。権限エラーの文言に IAM アクションを示す。
    Op(&'static str),
    /// バケットへのアクセス（HeadBucket など）。
    Bucket,
    /// AssumeRole。
    AssumeRole,
    /// GetCallerIdentity。
    Caller,
    /// Cost Explorer。
    Cost,
    /// CloudWatch。
    Metrics,
}

/// エラーの材料。SDK のエラー型に依存しない形にしてから分類する（テストしやすくするため）。
#[derive(Debug, Default, Clone)]
pub struct ErrorParts {
    pub code: Option<String>,
    pub message: Option<String>,
    pub status: Option<u16>,
    pub request_id: Option<String>,
    pub kind: TransportKind,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    #[default]
    Service,
    Timeout,
    Network,
    Credentials,
    Other,
}

pub fn classify<E>(err: &SdkError<E, HttpResponse>, ctx: Ctx) -> CoreError
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    classify_parts(parts_of(err), ctx)
}

pub fn parts_of<E>(err: &SdkError<E, HttpResponse>) -> ErrorParts
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    match err {
        SdkError::ServiceError(se) => {
            let meta = se.err().meta();
            ErrorParts {
                code: se.err().code().map(str::to_string),
                message: se.err().message().map(str::to_string),
                status: Some(se.raw().status().as_u16()),
                request_id: meta.extra("aws_request_id").map(str::to_string),
                kind: TransportKind::Service,
            }
        }
        SdkError::TimeoutError(_) => ErrorParts {
            kind: TransportKind::Timeout,
            ..Default::default()
        },
        SdkError::DispatchFailure(df) => {
            let kind = if df.is_timeout() {
                TransportKind::Timeout
            } else if source_chain_has_credentials_error(err) {
                TransportKind::Credentials
            } else {
                TransportKind::Network
            };
            ErrorParts {
                kind,
                message: Some(format!("{df:?}")),
                ..Default::default()
            }
        }
        SdkError::ResponseError(re) => ErrorParts {
            kind: TransportKind::Network,
            status: Some(re.raw().status().as_u16()),
            ..Default::default()
        },
        other => ErrorParts {
            kind: if source_chain_has_credentials_error(other) {
                TransportKind::Credentials
            } else {
                TransportKind::Other
            },
            message: Some(format!(
                "{}",
                aws_smithy_types::error::display::DisplayErrorContext(other)
            )),
            ..Default::default()
        },
    }
}

fn source_chain_has_credentials_error(err: &(dyn std::error::Error + 'static)) -> bool {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = cur {
        if e.is::<aws_credential_types::provider::error::CredentialsError>() {
            return true;
        }
        cur = e.source();
    }
    false
}

pub fn classify_parts(p: ErrorParts, ctx: Ctx) -> CoreError {
    let detail = {
        let mut d = Vec::new();
        if let Some(c) = &p.code {
            d.push(format!("code={c}"));
        }
        if let Some(s) = p.status {
            d.push(format!("status={s}"));
        }
        if let Some(r) = &p.request_id {
            d.push(format!("requestId={r}"));
        }
        if let Some(m) = &p.message {
            d.push(format!("message={m}"));
        }
        d.join(", ")
    };
    let with = |e: CoreError| {
        if detail.is_empty() {
            e
        } else {
            e.detail(detail.clone())
        }
    };

    match p.kind {
        TransportKind::Timeout => return with(CoreError::new(ErrorCode::Timeout)),
        TransportKind::Network => return with(CoreError::new(ErrorCode::Network)),
        TransportKind::Credentials => {
            let code = if ctx == Ctx::AssumeRole {
                ErrorCode::RoleAssumeDenied
            } else {
                ErrorCode::CredentialsExpired
            };
            return with(CoreError::new(code));
        }
        TransportKind::Other => return with(CoreError::new(ErrorCode::Internal)),
        TransportKind::Service => {}
    }

    let code = p.code.as_deref().unwrap_or("");
    let message = p.message.as_deref().unwrap_or("");
    let status = p.status.unwrap_or(0);

    let access_denied = || -> CoreError {
        let lower = message.to_ascii_lowercase();
        if lower.contains("mfa") {
            return CoreError::new(ErrorCode::MfaDeleteRequired);
        }
        if lower.contains("object lock")
            || lower.contains("objectlock")
            || lower.contains("retention")
        {
            return CoreError::new(ErrorCode::ObjectLocked);
        }
        match ctx {
            Ctx::Bucket => CoreError::new(ErrorCode::BucketAccessDenied),
            Ctx::AssumeRole => CoreError::new(ErrorCode::RoleAssumeDenied),
            Ctx::Cost => CoreError::with_message(
                ErrorCode::CostUnavailable,
                "Cost Explorer にアクセスする権限がありません（ce:GetCostAndUsage）",
            ),
            Ctx::Metrics => CoreError::with_message(
                ErrorCode::MetricsUnavailable,
                "CloudWatch のメトリクスを取得する権限がありません（cloudwatch:GetMetricData）",
            ),
            Ctx::Caller => CoreError::new(ErrorCode::CredentialsInvalid),
            Ctx::Op(action) => CoreError::with_message(
                ErrorCode::AccessDenied,
                format!("この操作を行う権限がありません（{action}）"),
            ),
        }
    };

    let err = match code {
        "InvalidAccessKeyId"
        | "SignatureDoesNotMatch"
        | "InvalidClientTokenId"
        | "UnrecognizedClientException"
        | "InvalidSignatureException" => CoreError::new(ErrorCode::CredentialsInvalid),
        "ExpiredToken" | "ExpiredTokenException" | "TokenRefreshRequired" => {
            CoreError::new(ErrorCode::CredentialsExpired)
        }
        "AccessDenied" | "AccessDeniedException" | "AllAccessDisabled" => access_denied(),
        "NoSuchBucket" => CoreError::new(ErrorCode::BucketNotFound),
        "NoSuchKey" | "NoSuchVersion" | "NoSuchUpload" => CoreError::new(ErrorCode::NotFound),
        "InvalidObjectState" => CoreError::new(ErrorCode::InvalidObjectState),
        "RestoreAlreadyInProgress" => CoreError::new(ErrorCode::RestoreInProgress),
        "GlacierExpeditedRetrievalNotAvailable" => CoreError::new(ErrorCode::ExpeditedUnavailable),
        "PreconditionFailed" => CoreError::new(ErrorCode::PreconditionFailed),
        "SlowDown"
        | "Throttling"
        | "ThrottlingException"
        | "TooManyRequestsException"
        | "RequestLimitExceeded"
        | "LimitExceededException" => CoreError::new(ErrorCode::SlowDown),
        "RequestTimeout" | "RequestTimeTooSkewed" => CoreError::new(ErrorCode::Timeout),
        "DataUnavailableException"
        | "BillExpirationException"
        | "OptInRequiredException"
        | "UnresolvableUsageUnitException" => {
            CoreError::with_message(ErrorCode::CostUnavailable, cost_message(code))
        }
        _ => match status {
            403 => access_denied(),
            404 if ctx == Ctx::Bucket => CoreError::new(ErrorCode::BucketNotFound),
            404 => CoreError::new(ErrorCode::NotFound),
            412 => CoreError::new(ErrorCode::PreconditionFailed),
            503 => CoreError::new(ErrorCode::SlowDown),
            500..=599 => CoreError::new(ErrorCode::Network),
            _ if ctx == Ctx::Cost => {
                CoreError::with_message(ErrorCode::CostUnavailable, cost_message(code))
            }
            _ => CoreError::new(ErrorCode::Internal),
        },
    };
    with(err)
}

fn cost_message(code: &str) -> String {
    match code {
        "DataUnavailableException" => "コスト情報を取得できません（データがまだありません）".into(),
        "OptInRequiredException" => {
            "コスト情報を取得できません（Cost Explorer が有効化されていません）".into()
        }
        _ => ErrorCode::CostUnavailable.default_message().to_string(),
    }
}

/// 一時認証情報の期限切れ（`ExpiredToken`）の応答か。S3・CloudWatch（XML）と Cost Explorer・Price List（JSON）の
/// どちらの形式でも判定できるよう、本文と `x-amzn-ErrorType` ヘッダーを見る（01 §7.1）。
pub fn is_expired_token_response(
    status: u16,
    error_type: Option<&str>,
    body: Option<&[u8]>,
) -> bool {
    if !(status == 400 || status == 403) {
        return false;
    }
    const CODES: [&[u8]; 2] = [b"ExpiredToken", b"TokenRefreshRequired"];
    let contains =
        |haystack: &[u8], needle: &[u8]| haystack.windows(needle.len()).any(|w| w == needle);
    error_type.is_some_and(|t| CODES.iter().any(|c| contains(t.as_bytes(), c)))
        || body.is_some_and(|b| CODES.iter().any(|c| contains(b, c)))
}

/// HeadBucket などの 301 応答から正しいリージョンを取り出す（04 §2.2）。
pub fn bucket_region_hint<E>(err: &SdkError<E, HttpResponse>) -> Option<String> {
    let raw = match err {
        SdkError::ServiceError(se) => se.raw(),
        SdkError::ResponseError(re) => re.raw(),
        _ => return None,
    };
    let status = raw.status().as_u16();
    if !(status == 301 || status == 400 || status == 403) {
        return None;
    }
    raw.headers().get("x-amz-bucket-region").map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(code: &str, status: u16) -> ErrorParts {
        ErrorParts {
            code: (!code.is_empty()).then(|| code.to_string()),
            status: Some(status),
            request_id: Some("REQ1".into()),
            ..Default::default()
        }
    }

    #[test]
    fn maps_credential_errors() {
        for code in [
            "InvalidAccessKeyId",
            "SignatureDoesNotMatch",
            "InvalidClientTokenId",
        ] {
            assert_eq!(
                classify_parts(svc(code, 403), Ctx::Op("s3:GetObject")).code,
                ErrorCode::CredentialsInvalid
            );
        }
        assert_eq!(
            classify_parts(svc("ExpiredToken", 400), Ctx::Bucket).code,
            ErrorCode::CredentialsExpired
        );
    }

    #[test]
    fn maps_access_denied_by_context() {
        assert_eq!(
            classify_parts(svc("AccessDenied", 403), Ctx::Bucket).code,
            ErrorCode::BucketAccessDenied
        );
        assert_eq!(
            classify_parts(svc("AccessDenied", 403), Ctx::AssumeRole).code,
            ErrorCode::RoleAssumeDenied
        );
        assert_eq!(
            classify_parts(svc("AccessDeniedException", 400), Ctx::Cost).code,
            ErrorCode::CostUnavailable
        );
        let op = classify_parts(svc("AccessDenied", 403), Ctx::Op("s3:DeleteObjectVersion"));
        assert_eq!(op.code, ErrorCode::AccessDenied);
        assert!(op.message.contains("s3:DeleteObjectVersion"));
        // HEAD はコードなしの 403 を返す
        assert_eq!(
            classify_parts(svc("", 403), Ctx::Bucket).code,
            ErrorCode::BucketAccessDenied
        );
    }

    #[test]
    fn maps_not_found() {
        assert_eq!(
            classify_parts(svc("NoSuchBucket", 404), Ctx::Op("s3:ListBucket")).code,
            ErrorCode::BucketNotFound
        );
        assert_eq!(
            classify_parts(svc("", 404), Ctx::Bucket).code,
            ErrorCode::BucketNotFound
        );
        assert_eq!(
            classify_parts(svc("", 404), Ctx::Op("s3:GetObject")).code,
            ErrorCode::NotFound
        );
        assert_eq!(
            classify_parts(svc("NoSuchVersion", 404), Ctx::Op("s3:GetObject")).code,
            ErrorCode::NotFound
        );
    }

    #[test]
    fn maps_archive_and_restore_errors() {
        assert_eq!(
            classify_parts(svc("InvalidObjectState", 403), Ctx::Op("s3:GetObject")).code,
            ErrorCode::InvalidObjectState
        );
        assert_eq!(
            classify_parts(
                svc("RestoreAlreadyInProgress", 409),
                Ctx::Op("s3:RestoreObject")
            )
            .code,
            ErrorCode::RestoreInProgress
        );
        assert_eq!(
            classify_parts(
                svc("GlacierExpeditedRetrievalNotAvailable", 503),
                Ctx::Op("s3:RestoreObject")
            )
            .code,
            ErrorCode::ExpeditedUnavailable
        );
    }

    #[test]
    fn maps_throttling_and_preconditions() {
        assert_eq!(
            classify_parts(svc("SlowDown", 503), Ctx::Op("s3:PutObject")).code,
            ErrorCode::SlowDown
        );
        assert_eq!(
            classify_parts(svc("", 503), Ctx::Op("s3:PutObject")).code,
            ErrorCode::SlowDown
        );
        assert_eq!(
            classify_parts(svc("PreconditionFailed", 412), Ctx::Op("s3:GetObject")).code,
            ErrorCode::PreconditionFailed
        );
        assert_eq!(
            classify_parts(svc("RequestTimeout", 400), Ctx::Op("s3:PutObject")).code,
            ErrorCode::Timeout
        );
    }

    #[test]
    fn maps_mfa_and_object_lock() {
        let mut p = svc("AccessDenied", 403);
        p.message = Some("Mfa Authentication must be used for this request".into());
        assert_eq!(
            classify_parts(p, Ctx::Op("s3:DeleteObjectVersion")).code,
            ErrorCode::MfaDeleteRequired
        );
        let mut p = svc("AccessDenied", 403);
        p.message = Some("Access Denied because object protected by object lock.".into());
        assert_eq!(
            classify_parts(p, Ctx::Op("s3:DeleteObjectVersion")).code,
            ErrorCode::ObjectLocked
        );
    }

    #[test]
    fn maps_cost_explorer_errors() {
        assert_eq!(
            classify_parts(svc("DataUnavailableException", 400), Ctx::Cost).code,
            ErrorCode::CostUnavailable
        );
    }

    #[test]
    fn maps_transport_failures() {
        let net = ErrorParts {
            kind: TransportKind::Network,
            ..Default::default()
        };
        let err = classify_parts(net, Ctx::Op("s3:ListBucket"));
        assert_eq!(err.code, ErrorCode::Network);
        assert!(err.code.retryable());
        let t = ErrorParts {
            kind: TransportKind::Timeout,
            ..Default::default()
        };
        assert_eq!(classify_parts(t, Ctx::Bucket).code, ErrorCode::Timeout);
        let c = ErrorParts {
            kind: TransportKind::Credentials,
            ..Default::default()
        };
        assert_eq!(
            classify_parts(c.clone(), Ctx::Op("s3:ListBucket")).code,
            ErrorCode::CredentialsExpired
        );
        assert_eq!(
            classify_parts(c, Ctx::AssumeRole).code,
            ErrorCode::RoleAssumeDenied
        );
    }

    #[test]
    fn detects_expired_token_responses() {
        let s3 = b"<Error><Code>ExpiredToken</Code><Message>The provided token has expired.</Message></Error>";
        assert!(is_expired_token_response(400, None, Some(s3)));
        let ce = br#"{"__type":"com.amazon.coral.service#ExpiredTokenException","message":"x"}"#;
        assert!(is_expired_token_response(400, None, Some(ce)));
        assert!(is_expired_token_response(
            403,
            Some("ExpiredTokenException:http://internal.amazon.com/coral/"),
            None
        ));
        assert!(!is_expired_token_response(
            403,
            None,
            Some(b"<Error><Code>AccessDenied</Code></Error>")
        ));
        assert!(!is_expired_token_response(500, None, Some(s3)));
    }

    #[test]
    fn detail_contains_request_id_but_not_secrets() {
        let err = classify_parts(svc("AccessDenied", 403), Ctx::Bucket);
        let detail = err.detail.unwrap();
        assert!(detail.contains("requestId=REQ1"));
        assert!(detail.contains("status=403"));
    }
}
