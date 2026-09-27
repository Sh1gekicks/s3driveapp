//! 送信したバイト数を数える本文（04 §4.2 の「進捗」）。

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};

use aws_smithy_types::body::SdkBody;
use aws_smithy_types::byte_stream::ByteStream;
use bytes::Bytes;
use http_body::{Body, Frame, SizeHint};

const CHUNK: usize = 64 * 1024;

/// メモリ上のデータを 64 KiB ずつ返し、返した分を `counter` に加える。
pub struct CountingBody {
    data: Bytes,
    counter: Arc<AtomicU64>,
    sent: Arc<AtomicU64>,
}

impl Body for CountingBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.data.is_empty() {
            return Poll::Ready(None);
        }
        let n = CHUNK.min(self.data.len());
        let chunk = self.data.split_to(n);
        self.counter.fetch_add(n as u64, Ordering::Relaxed);
        self.sent.fetch_add(n as u64, Ordering::Relaxed);
        Poll::Ready(Some(Ok(Frame::data(chunk))))
    }

    fn is_end_stream(&self) -> bool {
        self.data.is_empty()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.data.len() as u64)
    }
}

/// 再試行できる（SDK が作り直せる）数えながら送る本文。
///
/// SDK が再試行のために本文を作り直したときは、前の本文で数えた分を差し引いて二重に数えないようにする。
/// 戻り値の 2 つ目は、現在の本文が `counter` に加えたバイト数（失敗時に差し引くために使う）。
pub fn counting_stream(data: Bytes, counter: Arc<AtomicU64>) -> (ByteStream, Arc<AtomicU64>) {
    let sent_by_previous = Arc::new(AtomicU64::new(0));
    let sent = sent_by_previous.clone();
    let stream = ByteStream::new(SdkBody::retryable(move || {
        let previous = sent_by_previous.swap(0, Ordering::Relaxed);
        counter.fetch_sub(
            previous.min(counter.load(Ordering::Relaxed)),
            Ordering::Relaxed,
        );
        SdkBody::from_body_1_x(CountingBody {
            data: data.clone(),
            counter: counter.clone(),
            sent: sent_by_previous.clone(),
        })
    }));
    (stream, sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn counts_bytes_as_they_are_read() {
        let counter = Arc::new(AtomicU64::new(0));
        let data = Bytes::from(vec![7u8; 200 * 1024]);
        let (stream, sent) = counting_stream(data.clone(), counter.clone());
        let collected = stream.collect().await.unwrap().into_bytes();
        assert_eq!(collected, data);
        assert_eq!(counter.load(Ordering::Relaxed), 200 * 1024);
        assert_eq!(sent.load(Ordering::Relaxed), 200 * 1024);
    }
}
