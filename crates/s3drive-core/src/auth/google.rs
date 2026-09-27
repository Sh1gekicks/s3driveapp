//! Google の OAuth 2.0 認可コードフロー + PKCE（システムブラウザ + ループバック。07 §2）。

use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::Engine;
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreIdTokenClaims, CoreProviderMetadata,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, OAuth2TokenResponse,
    PkceCodeChallenge, RedirectUrl, RefreshToken, Scope,
};
use tokio_util::sync::CancellationToken;

use super::loopback::{Callback, LoopbackServer};
use super::{AuthProvider, SignInResult, UrlOpener};
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::UserSession;

const ISSUER: &str = "https://accounts.google.com";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const METADATA_TTL: Duration = Duration::from_secs(60 * 60);

/// ビルド時に埋め込む OAuth クライアントと許可リスト（07 §2.1、§2.3）。
#[derive(Debug, Clone)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
    pub allowed_emails: Vec<String>,
    pub allowed_domains: Vec<String>,
}

impl GoogleConfig {
    /// `S3DRIVE_GOOGLE_CLIENT_ID` などのビルド時の環境変数から作る。クライアント ID がなければ `None`。
    pub fn from_build_env() -> Option<Self> {
        let client_id = option_env!("S3DRIVE_GOOGLE_CLIENT_ID").filter(|s| !s.is_empty())?;
        Some(Self {
            client_id: client_id.to_string(),
            client_secret: option_env!("S3DRIVE_GOOGLE_CLIENT_SECRET")
                .unwrap_or_default()
                .to_string(),
            allowed_emails: split_list(option_env!("S3DRIVE_ALLOWED_EMAILS")),
            allowed_domains: split_list(option_env!("S3DRIVE_ALLOWED_DOMAINS")),
        })
    }

    /// 許可リストとの照合。リストが空なら制限なし。ドメインは `hd` クレームで確認する。
    pub fn is_allowed(&self, email: &str, hosted_domain: Option<&str>) -> bool {
        if self.allowed_emails.is_empty() && self.allowed_domains.is_empty() {
            return true;
        }
        let email = email.to_ascii_lowercase();
        self.allowed_emails.iter().any(|e| e == &email)
            || hosted_domain.is_some_and(|hd| {
                let hd = hd.to_ascii_lowercase();
                self.allowed_domains.iter().any(|d| d == &hd)
            })
    }
}

fn split_list(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

pub struct GoogleAuth {
    config: GoogleConfig,
    http: openidconnect::reqwest::Client,
    metadata: tokio::sync::Mutex<Option<(CoreProviderMetadata, Instant)>>,
}

impl GoogleAuth {
    pub fn new(config: GoogleConfig) -> CoreResult<Self> {
        let http = openidconnect::reqwest::ClientBuilder::new()
            // SSRF を避けるためリダイレクトに従わない（openidconnect の推奨）
            .redirect(openidconnect::reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| CoreError::internal(format!("http client: {e}")))?;
        Ok(Self {
            config,
            http,
            metadata: tokio::sync::Mutex::new(None),
        })
    }

    /// ディスカバリ文書と JWKS を取得する（1 時間キャッシュ）。
    async fn metadata(&self) -> CoreResult<CoreProviderMetadata> {
        let mut guard = self.metadata.lock().await;
        if let Some((m, at)) = guard.as_ref()
            && at.elapsed() < METADATA_TTL
        {
            return Ok(m.clone());
        }
        let issuer = IssuerUrl::new(ISSUER.to_string()).map_err(CoreError::internal)?;
        let metadata = CoreProviderMetadata::discover_async(issuer, &self.http)
            .await
            .map_err(|e| CoreError::new(ErrorCode::Network).detail(format!("discovery: {e}")))?;
        *guard = Some((metadata.clone(), Instant::now()));
        Ok(metadata)
    }

    fn session_from_claims(
        &self,
        claims: &CoreIdTokenClaims,
        raw_id_token: &str,
    ) -> CoreResult<UserSession> {
        let email = claims
            .email()
            .map(|e| e.as_str().to_string())
            .ok_or_else(|| CoreError::new(ErrorCode::AuthRequired).detail("no email claim"))?;
        if claims.email_verified() != Some(true) {
            return Err(CoreError::new(ErrorCode::AuthNotAllowed).detail("email not verified"));
        }
        let hd = hosted_domain(raw_id_token);
        if !self.config.is_allowed(&email, hd.as_deref()) {
            return Err(CoreError::new(ErrorCode::AuthNotAllowed));
        }
        let name = claims
            .name()
            .and_then(|n| n.get(None))
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| email.clone());
        Ok(UserSession::new(claims.subject().as_str(), email, name))
    }
}

/// 検証済みの ID トークンのペイロードから `hd`（Google Workspace のドメイン）を読む。
fn hosted_domain(raw_id_token: &str) -> Option<String> {
    let payload = raw_id_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("hd")?.as_str().map(str::to_string)
}

fn sign_in_failed(detail: impl std::fmt::Display) -> CoreError {
    CoreError::with_message(
        ErrorCode::AuthRequired,
        "サインインに失敗しました。もう一度お試しください",
    )
    .detail(detail.to_string())
}

#[async_trait]
impl AuthProvider for GoogleAuth {
    async fn sign_in(
        &self,
        open_url: &UrlOpener,
        cancel: CancellationToken,
    ) -> CoreResult<SignInResult> {
        let metadata = self.metadata().await?;
        let server = LoopbackServer::bind().await?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(self.config.client_id.clone()),
            Some(ClientSecret::new(self.config.client_secret.clone())),
        )
        .set_redirect_uri(RedirectUrl::new(server.redirect_uri()).map_err(CoreError::internal)?);

        // code_verifier は 64 文字（48 バイトの乱数の base64url）
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256_len(48);
        let (url, state, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                || CsrfToken::new(random_token()),
                || Nonce::new(random_token()),
            )
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .set_pkce_challenge(challenge)
            .add_extra_param("access_type", "offline")
            // サインアウト時にトークンを取り消すため、サインインのたびに同意を求めてリフレッシュトークンを得る
            .add_extra_param("prompt", "select_account consent")
            .url();

        open_url(url.as_str())?;
        let callback = server.wait(state.secret().clone(), cancel).await?;
        let code = match callback {
            Callback::Code(code) => code,
            Callback::Error(e) if e == "access_denied" => {
                return Err(CoreError::new(ErrorCode::AuthCanceled));
            }
            Callback::Error(e) => return Err(sign_in_failed(format!("oauth error: {e}"))),
        };

        let token = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(sign_in_failed)?
            .set_pkce_verifier(verifier)
            .request_async(&self.http)
            .await
            .map_err(|e| sign_in_failed(format!("token exchange: {e}")))?;
        let id_token = token
            .extra_fields()
            .id_token()
            .ok_or_else(|| sign_in_failed("no id_token"))?;
        let verifier = client.id_token_verifier();
        let claims = id_token
            .claims(&verifier, &nonce)
            .map_err(|e| sign_in_failed(format!("id_token: {e}")))?;
        let refresh_token = token
            .refresh_token()
            .map(|t| t.secret().clone())
            .ok_or_else(|| sign_in_failed("no refresh_token"))?;
        match self.session_from_claims(claims, &id_token.to_string()) {
            Ok(session) => Ok(SignInResult {
                session,
                refresh_token,
            }),
            Err(e) => {
                // 許可されていないアカウントのトークンは取り消す（04 §1.5）
                let _ = self.revoke(&refresh_token).await;
                Err(e)
            }
        }
    }

    async fn refresh(&self, refresh_token: &str) -> CoreResult<UserSession> {
        let metadata = self.metadata().await?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(self.config.client_id.clone()),
            Some(ClientSecret::new(self.config.client_secret.clone())),
        );
        let token = client
            .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
            .map_err(CoreError::internal)?
            .request_async(&self.http)
            .await
            .map_err(|e| {
                let text = e.to_string();
                if text.contains("invalid_grant") {
                    // 失効・取り消し（同意画面が「テスト」のときは 7 日で失効する）
                    CoreError::new(ErrorCode::AuthRequired).detail("invalid_grant")
                } else {
                    CoreError::new(ErrorCode::Network).detail(format!("refresh: {text}"))
                }
            })?;
        let id_token = token
            .extra_fields()
            .id_token()
            .ok_or_else(|| CoreError::new(ErrorCode::AuthRequired).detail("no id_token"))?;
        let verifier = client.id_token_verifier();
        // リフレッシュで得た ID トークンには nonce がない（07 §2.3）
        let claims = id_token
            .claims(&verifier, |_: Option<&Nonce>| Ok(()))
            .map_err(|e| {
                CoreError::new(ErrorCode::AuthRequired).detail(format!("id_token: {e}"))
            })?;
        self.session_from_claims(claims, &id_token.to_string())
    }

    async fn revoke(&self, refresh_token: &str) -> CoreResult<()> {
        self.http
            .post(REVOKE_URL)
            .form(&[("token", refresh_token)])
            .send()
            .await
            .map_err(|e| CoreError::new(ErrorCode::Network).detail(format!("revoke: {e}")))?;
        Ok(())
    }
}

/// 32 バイトの乱数（base64url）。state と nonce に使う。
fn random_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(emails: &[&str], domains: &[&str]) -> GoogleConfig {
        GoogleConfig {
            client_id: "id".into(),
            client_secret: String::new(),
            allowed_emails: emails.iter().map(|s| s.to_string()).collect(),
            allowed_domains: domains.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn allow_list_is_open_when_empty() {
        assert!(config(&[], &[]).is_allowed("anyone@gmail.com", None));
    }

    #[test]
    fn allow_list_matches_emails_and_hosted_domains() {
        let c = config(&["yuki@gmail.com"], &["acme.co.jp"]);
        assert!(c.is_allowed("Yuki@Gmail.com", None));
        assert!(c.is_allowed("taro@acme.co.jp", Some("acme.co.jp")));
        // hd クレームがない（個人アカウント）はドメインでは許可しない
        assert!(!c.is_allowed("taro@acme.co.jp", None));
        assert!(!c.is_allowed("eve@gmail.com", None));
    }

    #[test]
    fn splits_comma_separated_lists() {
        assert_eq!(
            split_list(Some(" A@x.com, ,b@y.com ")),
            vec!["a@x.com", "b@y.com"]
        );
        assert!(split_list(None).is_empty());
    }

    #[test]
    fn reads_hosted_domain_from_payload() {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"sub":"1","hd":"acme.co.jp"}"#);
        assert_eq!(
            hosted_domain(&format!("h.{payload}.s")).as_deref(),
            Some("acme.co.jp")
        );
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"sub":"1"}"#);
        assert_eq!(hosted_domain(&format!("h.{payload}.s")), None);
    }

    #[test]
    fn random_tokens_are_unique_and_long() {
        let a = random_token();
        assert_eq!(a.len(), 43);
        assert_ne!(a, random_token());
    }
}
