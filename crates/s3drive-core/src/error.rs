//! エラーの表現（01 §7.1、05 §5）。
//!
//! コアのエラーは [`CoreError`] で表し、IPC では [`AppError`] に変換して返す。
//! 表示する文言は DS の文言ルールに従った日本語とする。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// IPC で返すエラーコード（05 §5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ErrorCode {
    Network,
    Timeout,
    SlowDown,
    AuthRequired,
    AuthCanceled,
    AuthNotAllowed,
    CredentialsInvalid,
    CredentialsExpired,
    RoleAssumeDenied,
    BucketNotFound,
    BucketAccessDenied,
    AccessDenied,
    NotFound,
    AlreadyExists,
    InvalidName,
    InvalidObjectState,
    RestoreInProgress,
    ExpeditedUnavailable,
    PreconditionFailed,
    FileChanged,
    LocalIo,
    DiskFull,
    MfaDeleteRequired,
    ObjectLocked,
    CostUnavailable,
    MetricsUnavailable,
    Canceled,
    Internal,
}

impl ErrorCode {
    /// 既定の表示文言（05 §5 の「表示する文言」）。
    pub fn default_message(self) -> &'static str {
        match self {
            Self::Network => "接続できませんでした。ネットワークを確認してください",
            Self::Timeout => "応答がありませんでした",
            Self::SlowDown => "しばらくしてからもう一度お試しください",
            Self::AuthRequired => "もう一度サインインしてください",
            Self::AuthCanceled => "サインインがキャンセルされました",
            Self::AuthNotAllowed => "このアカウントは利用が許可されていません",
            Self::CredentialsInvalid => {
                "認証に失敗しました。アクセスキーとシークレットキーを確認してください"
            }
            Self::CredentialsExpired => "認証情報の有効期限が切れました",
            Self::RoleAssumeDenied => {
                "ロールを引き受けられませんでした。信頼ポリシーと権限を確認してください"
            }
            Self::BucketNotFound => "バケットが見つかりません",
            Self::BucketAccessDenied => "このバケットへのアクセス権がありません",
            Self::AccessDenied => "この操作を行う権限がありません",
            Self::NotFound => "項目が見つかりません。ほかの操作で削除された可能性があります",
            Self::AlreadyExists => "同じ名前の項目があります",
            Self::InvalidName => "この名前は使用できません",
            Self::InvalidObjectState => "取り出しが必要です",
            Self::RestoreInProgress => "取り出し中です。完了したら通知します",
            Self::ExpeditedUnavailable => {
                "迅速な取り出しは現在利用できません。標準を選んでください"
            }
            Self::PreconditionFailed => "項目が変更されたため中止しました",
            Self::FileChanged => "送信中にファイルが変更されました",
            Self::LocalIo => "ファイルを読み書きできませんでした",
            Self::DiskFull => "ディスクの空き容量が足りません",
            Self::MfaDeleteRequired => {
                "MFA Delete が有効なため、アプリからはバージョンを削除できません"
            }
            Self::ObjectLocked => "このバージョンは保護期間中のため削除できません",
            Self::CostUnavailable => "コスト情報を取得できません",
            Self::MetricsUnavailable => "メトリクスを取得できません",
            Self::Canceled => "キャンセルしました",
            Self::Internal => "予期しないエラーが発生しました",
        }
    }

    /// 再試行で解決しうるか（05 §5 の「再試行」列）。
    pub fn retryable(self) -> bool {
        matches!(
            self,
            Self::Network
                | Self::Timeout
                | Self::SlowDown
                | Self::PreconditionFailed
                | Self::FileChanged
        )
    }
}

/// コア内部のエラー。
#[derive(Debug, Clone, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct CoreError {
    pub code: ErrorCode,
    pub message: String,
    /// AWS のエラーコード・リクエスト ID・HTTP ステータスなど。秘密情報は含めない。
    pub detail: Option<String>,
}

impl CoreError {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            message: code.default_message().to_string(),
            detail: None,
        }
    }

    pub fn with_message(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn internal(detail: impl std::fmt::Display) -> Self {
        Self::new(ErrorCode::Internal).detail(detail.to_string())
    }

    pub fn canceled() -> Self {
        Self::new(ErrorCode::Canceled)
    }

    pub fn is_canceled(&self) -> bool {
        self.code == ErrorCode::Canceled
    }

    /// ローカルファイルの読み書きエラー。パスは表示文言に含める（05 §5 の LOCAL_IO）。
    pub fn local_io(path: &std::path::Path, err: &std::io::Error) -> Self {
        let code = if is_disk_full(err) {
            ErrorCode::DiskFull
        } else {
            ErrorCode::LocalIo
        };
        let message = if code == ErrorCode::LocalIo {
            format!(
                "ファイルを読み書きできませんでした（{}）",
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string())
            )
        } else {
            code.default_message().to_string()
        };
        Self::with_message(code, message).detail(err.to_string())
    }
}

fn is_disk_full(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::StorageFull || err.raw_os_error() == Some(28)
}

impl From<rusqlite::Error> for CoreError {
    fn from(err: rusqlite::Error) -> Self {
        Self::internal(format!("sqlite: {err}"))
    }
}

impl From<r2d2::Error> for CoreError {
    fn from(err: r2d2::Error) -> Self {
        Self::internal(format!("sqlite pool: {err}"))
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        Self::internal(format!("json: {err}"))
    }
}

impl From<tokio::task::JoinError> for CoreError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::internal(format!("task: {err}"))
    }
}

pub type CoreResult<T> = Result<T, CoreError>;

/// IPC で返すエラー（05 §2 の `AppError`）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub retryable: bool,
}

impl From<CoreError> for AppError {
    fn from(err: CoreError) -> Self {
        Self {
            code: err.code,
            retryable: err.code.retryable(),
            message: err.message,
            detail: err.detail,
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_serializes_in_screaming_snake_case() {
        let err: AppError = CoreError::new(ErrorCode::BucketNotFound).into();
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "BUCKET_NOT_FOUND");
        assert_eq!(json["message"], "バケットが見つかりません");
        assert_eq!(json["retryable"], false);
        assert!(json.get("detail").is_none());
    }

    #[test]
    fn network_errors_are_retryable() {
        let err: AppError = CoreError::new(ErrorCode::Network).into();
        assert!(err.retryable);
    }

    #[test]
    fn disk_full_is_detected_from_errno() {
        let err = std::io::Error::from_raw_os_error(28);
        let core = CoreError::local_io(std::path::Path::new("/tmp/a.bin"), &err);
        assert_eq!(core.code, ErrorCode::DiskFull);
    }

    #[test]
    fn local_io_message_contains_file_name() {
        let err = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let core = CoreError::local_io(std::path::Path::new("/Users/a/report.pdf"), &err);
        assert_eq!(core.code, ErrorCode::LocalIo);
        assert!(core.message.contains("report.pdf"));
    }
}
