//! ユーザ認証（Google。04 §1）。

pub mod google;
pub mod loopback;

use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::Core;
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::UserSession;
use crate::store::secrets::google_account;

/// サインインの待ち受けの上限（04 §1.2）。
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// 認可 URL を既定のブラウザで開く関数（Tauri の opener プラグインで実装する）。
pub type UrlOpener = dyn Fn(&str) -> CoreResult<()> + Send + Sync;

pub struct SignInResult {
    pub session: UserSession,
    pub refresh_token: String,
}

/// 認証の実装。本番は Google、E2E と開発用は固定のセッションを返す実装に差し替える（09 §2.5）。
#[async_trait]
pub trait AuthProvider: Send + Sync + 'static {
    async fn sign_in(
        &self,
        open_url: &UrlOpener,
        cancel: CancellationToken,
    ) -> CoreResult<SignInResult>;
    /// リフレッシュトークンから ID トークンを取り直して検証する。失効していれば `AUTH_REQUIRED`。
    async fn refresh(&self, refresh_token: &str) -> CoreResult<UserSession>;
    async fn revoke(&self, refresh_token: &str) -> CoreResult<()>;
}

/// 固定のセッションを返す実装（E2E テスト・Google の OAuth クライアント未設定の開発ビルド用）。
pub struct FixedAuth {
    session: UserSession,
}

impl FixedAuth {
    pub fn new(session: UserSession) -> Self {
        Self { session }
    }

    pub fn test_user() -> Self {
        Self::new(UserSession::new(
            "dev-user",
            "dev@example.com",
            "開発ユーザー",
        ))
    }
}

#[async_trait]
impl AuthProvider for FixedAuth {
    async fn sign_in(
        &self,
        _open_url: &UrlOpener,
        _cancel: CancellationToken,
    ) -> CoreResult<SignInResult> {
        Ok(SignInResult {
            session: self.session.clone(),
            refresh_token: "fixed".to_string(),
        })
    }

    async fn refresh(&self, _refresh_token: &str) -> CoreResult<UserSession> {
        Ok(self.session.clone())
    }

    async fn revoke(&self, _refresh_token: &str) -> CoreResult<()> {
        Ok(())
    }
}

/// Google の OAuth クライアントがビルド時に設定されていない場合の実装（リリースビルド）。
pub struct UnconfiguredAuth;

#[async_trait]
impl AuthProvider for UnconfiguredAuth {
    async fn sign_in(
        &self,
        _open_url: &UrlOpener,
        _cancel: CancellationToken,
    ) -> CoreResult<SignInResult> {
        Err(CoreError::with_message(
            ErrorCode::AuthRequired,
            "Google の OAuth クライアントが設定されていないため、サインインできません",
        ))
    }

    async fn refresh(&self, _refresh_token: &str) -> CoreResult<UserSession> {
        Err(CoreError::new(ErrorCode::AuthRequired))
    }

    async fn revoke(&self, _refresh_token: &str) -> CoreResult<()> {
        Ok(())
    }
}

impl Core {
    /// 起動時のセッション復元（04 §1.3）。サインインが必要なら `None`。
    pub async fn auth_restore(&self) -> CoreResult<Option<UserSession>> {
        if let Some(session) = self.session() {
            return Ok(Some(session));
        }
        let Some(sub) = self.0.settings.read(|f| f.last_account.clone()) else {
            return Ok(None);
        };
        let Some(token) = self.0.secrets.get(&google_account(&sub))? else {
            return Ok(None);
        };
        match self.0.auth.refresh(&token).await {
            Ok(session) => {
                self.activate(&session)?;
                Ok(Some(session))
            }
            Err(e) if e.code == ErrorCode::AuthRequired => {
                self.0.secrets.delete(&google_account(&sub))?;
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// ブラウザでサインインし、完了まで待つ（最大 5 分）。
    pub async fn auth_sign_in(&self, open_url: &UrlOpener) -> CoreResult<UserSession> {
        let cancel = CancellationToken::new();
        if let Some(previous) = self
            .0
            .sign_in_cancel
            .lock()
            .unwrap()
            .replace(cancel.clone())
        {
            previous.cancel();
        }
        let result = tokio::time::timeout(
            SIGN_IN_TIMEOUT,
            self.0.auth.sign_in(open_url, cancel.clone()),
        )
        .await;
        self.0.sign_in_cancel.lock().unwrap().take();
        let result = match result {
            Ok(r) => r?,
            Err(_) => return Err(CoreError::new(ErrorCode::AuthCanceled).detail("timeout")),
        };
        if let Some(sub) = self.session().map(|s| s.sub)
            && sub != result.session.sub
        {
            // 別のアカウントに切り替える場合は、前のアカウントの状態を破棄する
            self.reset_runtime_state();
        }
        self.0
            .secrets
            .set(&google_account(&result.session.sub), &result.refresh_token)?;
        self.activate(&result.session)?;
        Ok(result.session)
    }

    pub fn auth_cancel_sign_in(&self) {
        if let Some(cancel) = self.0.sign_in_cancel.lock().unwrap().take() {
            cancel.cancel();
        }
    }

    /// サインアウト（04 §1.4）。トークンの取り消し、ジョブのキャンセル、状態の破棄。接続は `sub` ごとに残す。
    pub async fn auth_sign_out(&self) -> CoreResult<()> {
        let Some(session) = self.session() else {
            return Ok(());
        };
        let account = google_account(&session.sub);
        if let Some(token) = self.0.secrets.get(&account)?
            && let Err(e) = self.0.auth.revoke(&token).await
        {
            log::warn!("トークンを取り消せませんでした: {e}");
        }
        self.0.secrets.delete(&account)?;
        self.reset_runtime_state();
        Ok(())
    }

    fn activate(&self, session: &UserSession) -> CoreResult<()> {
        self.0.settings.update(|file| {
            file.last_account = Some(session.sub.clone());
            let account = file.accounts.entry(session.sub.clone()).or_default();
            account.profile.email = session.email.clone();
            account.profile.name = session.name.clone();
            Ok(())
        })?;
        self.set_session(Some(session.clone()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sign_in_restore_and_sign_out() {
        let (core, dir) = Core::for_tests(None).unwrap();
        assert_eq!(core.auth_restore().await.unwrap(), None);

        let session = core.auth_sign_in(&|_| Ok(())).await.unwrap();
        assert_eq!(core.session(), Some(session.clone()));

        // 再起動を想定して、同じ保存先から作り直す
        let secrets = core.0.secrets.clone();
        drop(core);
        let restarted = Core::new(crate::CoreConfig {
            data_dir: dir.path().to_path_buf(),
            secrets,
            auth: std::sync::Arc::new(FixedAuth::test_user()),
            endpoint_override: None,
            allow_connection_endpoints: false,
        })
        .unwrap();
        assert_eq!(restarted.auth_restore().await.unwrap(), Some(session));

        restarted.auth_sign_out().await.unwrap();
        assert_eq!(restarted.session(), None);
        assert_eq!(restarted.auth_restore().await.unwrap(), None);
    }
}
