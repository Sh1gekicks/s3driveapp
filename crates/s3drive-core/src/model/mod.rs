//! IPC で受け渡す DTO（serde + ts-rs）。型は `cargo test` で `src/lib/ipc/bindings` に書き出される。

mod common;
mod connection;
mod metrics;
mod object;
mod search;
mod settings;
mod transfer;

pub use common::*;
pub use connection::*;
pub use metrics::*;
pub use object::*;
pub use search::*;
pub use settings::*;
pub use transfer::*;

/// `null`（値を消す）と「指定なし」を区別するためのデシリアライザ。
pub(crate) fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}
