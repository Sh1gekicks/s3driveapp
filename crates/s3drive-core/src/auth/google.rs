//! Google の OAuth 2.0 認可コードフロー + PKCE（システムブラウザ + ループバック。07 §2）。

use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::Engine;
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreIdToken, CoreIdTokenClaims, CoreIdTokenVerifier,
    CoreProviderMetadata,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, NonceVerifier,
    OAuth2TokenResponse, PkceCodeChallenge, RedirectUrl, RefreshToken, Scope,
};
use tokio_util::sync::CancellationToken;

use super::loopback::{Callback, LoopbackServer};
use super::{AuthProvider, SignInResult, UrlOpener};
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::UserSession;

const ISSUER: &str = "https://accounts.google.com";
/// ID トークンの `exp` の猶予（07 §2.3）。
const EXPIRY_LEEWAY: chrono::Duration = chrono::Duration::seconds(60);
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

    /// ID トークンを検証し、セッションを作る（07 §2.3）。
    ///
    /// 署名・`iss`（ディスカバリ文書の issuer と一致）・`aud`・`nonce` は openidconnect で、`exp` は 60 秒の猶予を
    /// 付けて確認する。`email_verified` と許可リストは [`Self::session_from_claims`] で確認する。
    /// `invalid` は検証に失敗したときのエラー（サインインと復元で文言が違う）。
    fn verified_session(
        &self,
        verifier: CoreIdTokenVerifier<'_>,
        id_token: &CoreIdToken,
        nonce: impl NonceVerifier,
        invalid: fn(String) -> CoreError,
    ) -> CoreResult<UserSession> {
        let verifier = verifier.set_time_fn(|| chrono::Utc::now() - EXPIRY_LEEWAY);
        let claims = id_token
            .claims(&verifier, nonce)
            .map_err(|e| invalid(format!("id_token: {e}")))?;
        self.session_from_claims(claims, &id_token.to_string())
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
        let refresh_token = token
            .refresh_token()
            .map(|t| t.secret().clone())
            .ok_or_else(|| sign_in_failed("no refresh_token"))?;
        match self.verified_session(client.id_token_verifier(), id_token, &nonce, sign_in_failed) {
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
        // リフレッシュで得た ID トークンには nonce がない（07 §2.3）
        self.verified_session(
            client.id_token_verifier(),
            id_token,
            |_: Option<&Nonce>| Ok(()),
            |detail| CoreError::new(ErrorCode::AuthRequired).detail(detail),
        )
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

    mod id_token {
        use openidconnect::core::{
            CoreGenderClaim, CoreHmacKey, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
        };
        use openidconnect::{Audience, EmptyAdditionalClaims, EndUserEmail, StandardClaims};
        use openidconnect::{EndUserName, LocalizedClaim, SubjectIdentifier};

        use super::*;

        // テスト用の鍵（HS256。クライアントシークレットを共有鍵にする）
        const CLIENT_ID: &str = "client-id.apps.googleusercontent.com";
        const SECRET: &str = "test-client-secret-for-hs256-signing";

        struct Token {
            issuer: &'static str,
            audience: &'static str,
            expires_in: chrono::Duration,
            nonce: Option<&'static str>,
            email_verified: bool,
        }

        impl Default for Token {
            fn default() -> Self {
                Self {
                    issuer: "https://accounts.google.com",
                    audience: CLIENT_ID,
                    expires_in: chrono::Duration::minutes(10),
                    nonce: Some("n-123"),
                    email_verified: true,
                }
            }
        }

        fn sign(t: Token) -> CoreIdToken {
            let now = chrono::Utc::now();
            let mut name = LocalizedClaim::new();
            name.insert(None, EndUserName::new("田中 優希".into()));
            let standard =
                StandardClaims::<CoreGenderClaim>::new(SubjectIdentifier::new("1234567890".into()))
                    .set_email(Some(EndUserEmail::new("yuki@example.com".into())))
                    .set_email_verified(Some(t.email_verified))
                    .set_name(Some(name));
            let claims = CoreIdTokenClaims::new(
                IssuerUrl::new(t.issuer.into()).unwrap(),
                vec![Audience::new(t.audience.into())],
                now + t.expires_in,
                now - chrono::Duration::minutes(1),
                standard,
                EmptyAdditionalClaims {},
            )
            .set_nonce(t.nonce.map(|n| Nonce::new(n.into())));
            CoreIdToken::new(
                claims,
                &CoreHmacKey::new(SECRET.as_bytes()),
                CoreJwsSigningAlgorithm::HmacSha256,
                None,
                None,
            )
            .unwrap()
        }

        fn auth(emails: &[&str]) -> GoogleAuth {
            GoogleAuth::new(GoogleConfig {
                client_id: CLIENT_ID.into(),
                client_secret: SECRET.into(),
                allowed_emails: emails.iter().map(|s| s.to_string()).collect(),
                allowed_domains: vec![],
            })
            .unwrap()
        }

        fn verify(auth: &GoogleAuth, token: &CoreIdToken) -> CoreResult<UserSession> {
            let verifier = CoreIdTokenVerifier::new_confidential_client(
                ClientId::new(CLIENT_ID.into()),
                ClientSecret::new(SECRET.into()),
                IssuerUrl::new(ISSUER.into()).unwrap(),
                CoreJsonWebKeySet::default(),
            )
            .set_allowed_algs(vec![CoreJwsSigningAlgorithm::HmacSha256]);
            auth.verified_session(verifier, token, &Nonce::new("n-123".into()), sign_in_failed)
        }

        #[test]
        fn accepts_a_valid_token() {
            let session = verify(&auth(&[]), &sign(Token::default())).unwrap();
            assert_eq!(session.sub, "1234567890");
            assert_eq!(session.email, "yuki@example.com");
            assert_eq!(session.name, "田中 優希");
        }

        #[test]
        fn rejects_other_issuers() {
            let token = sign(Token {
                issuer: "https://evil.example.com",
                ..Token::default()
            });
            assert_eq!(
                verify(&auth(&[]), &token).unwrap_err().code,
                ErrorCode::AuthRequired
            );
        }

        #[test]
        fn rejects_other_audiences_and_nonces() {
            let token = sign(Token {
                audience: "someone-else",
                ..Token::default()
            });
            assert!(verify(&auth(&[]), &token).is_err());
            let token = sign(Token {
                nonce: Some("replayed"),
                ..Token::default()
            });
            assert!(verify(&auth(&[]), &token).is_err());
        }

        #[test]
        fn allows_60_seconds_of_clock_skew_after_expiry() {
            let token = sign(Token {
                expires_in: chrono::Duration::seconds(-30),
                ..Token::default()
            });
            assert!(verify(&auth(&[]), &token).is_ok());
            let token = sign(Token {
                expires_in: chrono::Duration::seconds(-120),
                ..Token::default()
            });
            assert!(verify(&auth(&[]), &token).is_err());
        }

        #[test]
        fn requires_a_verified_email_on_the_allow_list() {
            let token = sign(Token {
                email_verified: false,
                ..Token::default()
            });
            assert_eq!(
                verify(&auth(&[]), &token).unwrap_err().code,
                ErrorCode::AuthNotAllowed
            );
            let token = sign(Token::default());
            assert_eq!(
                verify(&auth(&["other@example.com"]), &token)
                    .unwrap_err()
                    .code,
                ErrorCode::AuthNotAllowed
            );
            assert!(verify(&auth(&["yuki@example.com"]), &token).is_ok());
        }
    }

    #[test]
    fn random_tokens_are_unique_and_long() {
        let a = random_token();
        assert_eq!(a.len(), 43);
        assert_ne!(a, random_token());
    }
}
