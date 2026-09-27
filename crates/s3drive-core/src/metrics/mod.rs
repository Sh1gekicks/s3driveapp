//! 利用容量・コスト・単価（04 §12〜§13）。

pub mod cost;
pub mod pricing;
pub mod storage;

use crate::Core;
use crate::error::CoreResult;

impl Core {
    /// 設定「キャッシュを削除」（06 §7）。
    pub async fn clear_metrics_cache(&self) -> CoreResult<()> {
        crate::store::metrics_cache::clear_all(&self.0.db).await
    }
}
