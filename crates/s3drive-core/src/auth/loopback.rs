//! OAuth のリダイレクトを受けるループバックの最小 HTTP サーバー（07 §2.2）。
//!
//! - `127.0.0.1` のみにバインドする（`0.0.0.0` にはしない）
//! - 1 回の要求だけを受け付けて終了する。`state` が一致しない要求は破棄する
//! - 応答ページは外部リソースを読み込まない最小の HTML とし、`Cache-Control: no-store` を付ける

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::error::{CoreError, CoreResult, ErrorCode};

/// リダイレクトで受け取った値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Callback {
    Code(String),
    /// Google が返したエラー（`access_denied` など）。
    Error(String),
}

pub struct LoopbackServer {
    listener: TcpListener,
    addr: SocketAddr,
}

const DONE_PAGE: &str = "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><title>S3 Drive</title>\
<body style=\"font-family:-apple-system,sans-serif;text-align:center;padding-top:80px\">\
<h1 style=\"font-size:20px\">サインインが完了しました</h1><p>S3 Drive に戻ってください。このタブは閉じてかまいません。</p></body></html>";

const ERROR_PAGE: &str = "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><title>S3 Drive</title>\
<body style=\"font-family:-apple-system,sans-serif;text-align:center;padding-top:80px\">\
<h1 style=\"font-size:20px\">サインインできませんでした</h1><p>S3 Drive に戻って、もう一度お試しください。</p></body></html>";

impl LoopbackServer {
    /// 空きポートで待ち受ける。
    pub async fn bind() -> CoreResult<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| CoreError::internal(format!("loopback bind: {e}")))?;
        let addr = listener
            .local_addr()
            .map_err(|e| CoreError::internal(format!("loopback addr: {e}")))?;
        Ok(Self { listener, addr })
    }

    /// Google に登録するリダイレクト URI（`http://127.0.0.1:{port}`）。
    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}", self.addr.port())
    }

    /// `state` が一致する要求を 1 回受け取るまで待つ。タイムアウトとキャンセルは呼び出し側で扱う。
    pub async fn wait(
        self,
        expected_state: String,
        cancel: CancellationToken,
    ) -> CoreResult<Callback> {
        let (tx, rx) = oneshot::channel::<Callback>();
        let tx = Arc::new(Mutex::new(Some(tx)));
        let expected = Arc::new(expected_state);
        let accept = async {
            loop {
                let (stream, _) = match self.listener.accept().await {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("loopback accept: {e}");
                        continue;
                    }
                };
                let tx = tx.clone();
                let expected = expected.clone();
                tokio::spawn(async move {
                    let service = service_fn(move |req: Request<Incoming>| {
                        let tx = tx.clone();
                        let expected = expected.clone();
                        async move { Ok::<_, Infallible>(handle(req, &expected, &tx)) }
                    });
                    let _ = http1::Builder::new()
                        .keep_alive(false)
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        };
        tokio::select! {
            result = rx => result.map_err(|_| CoreError::new(ErrorCode::AuthCanceled)),
            _ = accept => Err(CoreError::new(ErrorCode::AuthCanceled)),
            _ = cancel.cancelled() => Err(CoreError::new(ErrorCode::AuthCanceled)),
        }
    }
}

type Sender = Arc<Mutex<Option<oneshot::Sender<Callback>>>>;

fn handle(req: Request<Incoming>, expected_state: &str, tx: &Sender) -> Response<Full<Bytes>> {
    let query = req.uri().query().unwrap_or("");
    let params: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let get = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());

    if req.uri().path() != "/" || (get("code").is_none() && get("error").is_none()) {
        return page(StatusCode::NOT_FOUND, ERROR_PAGE);
    }
    // state が一致しない要求は破棄する（CSRF 対策）
    if get("state").as_deref() != Some(expected_state) {
        log::warn!("OAuth のリダイレクトの state が一致しません");
        return page(StatusCode::BAD_REQUEST, ERROR_PAGE);
    }
    let callback = match (get("code"), get("error")) {
        (Some(code), _) => Callback::Code(code),
        (None, Some(error)) => Callback::Error(error),
        _ => unreachable!(),
    };
    let ok = matches!(callback, Callback::Code(_));
    if let Some(sender) = tx.lock().unwrap().take() {
        let _ = sender.send(callback);
    }
    if ok {
        page(StatusCode::OK, DONE_PAGE)
    } else {
        page(StatusCode::OK, ERROR_PAGE)
    }
}

fn page(status: StatusCode, body: &'static str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Cache-Control", "no-store")
        .header(
            "Content-Security-Policy",
            "default-src 'none'; style-src 'unsafe-inline'",
        )
        .body(Full::new(Bytes::from_static(body.as_bytes())))
        .expect("valid response")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn get(addr: &str, path: &str) -> String {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        let mut buf = String::new();
        stream.read_to_string(&mut buf).await.unwrap();
        buf
    }

    #[tokio::test]
    async fn accepts_only_matching_state() {
        let server = LoopbackServer::bind().await.unwrap();
        let uri = server.redirect_uri();
        assert!(uri.starts_with("http://127.0.0.1:"));
        let addr = uri.trim_start_matches("http://").to_string();
        let wait = tokio::spawn(server.wait("s1".into(), CancellationToken::new()));

        let bad = get(&addr, "/?code=evil&state=other").await;
        assert!(bad.starts_with("HTTP/1.1 400"));
        let favicon = get(&addr, "/favicon.ico").await;
        assert!(favicon.starts_with("HTTP/1.1 404"));
        let ok = get(&addr, "/?code=abc&state=s1").await;
        assert!(ok.starts_with("HTTP/1.1 200"));
        assert!(ok.to_ascii_lowercase().contains("cache-control: no-store"));

        assert_eq!(wait.await.unwrap().unwrap(), Callback::Code("abc".into()));
    }

    #[tokio::test]
    async fn reports_denied_consent_and_cancellation() {
        let server = LoopbackServer::bind().await.unwrap();
        let addr = server
            .redirect_uri()
            .trim_start_matches("http://")
            .to_string();
        let wait = tokio::spawn(server.wait("s".into(), CancellationToken::new()));
        get(&addr, "/?error=access_denied&state=s").await;
        assert_eq!(
            wait.await.unwrap().unwrap(),
            Callback::Error("access_denied".into())
        );

        let server = LoopbackServer::bind().await.unwrap();
        let cancel = CancellationToken::new();
        let wait = tokio::spawn(server.wait("s".into(), cancel.clone()));
        cancel.cancel();
        assert_eq!(
            wait.await.unwrap().unwrap_err().code,
            ErrorCode::AuthCanceled
        );
    }
}
