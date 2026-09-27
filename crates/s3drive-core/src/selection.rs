//! ファイル選択・ドロップで得たローカルパスの保管（05 §3.9、07 §5.1）。
//!
//! フロントエンドにはパスを渡さず、選択 ID と表示用の名前・サイズだけを渡す。選択は 10 分で失効する。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::{Selection, SelectionItem};
use crate::util::key;

pub const SELECTION_TTL: Duration = Duration::from_secs(10 * 60);

struct Stored {
    paths: Vec<PathBuf>,
    created: Instant,
}

#[derive(Default)]
pub struct SelectionRegistry {
    items: Mutex<HashMap<String, Stored>>,
}

impl SelectionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// パスを保管し、表示用の情報を返す。
    pub fn register(&self, paths: Vec<PathBuf>) -> Selection {
        let items = paths
            .iter()
            .map(|p| {
                let meta = std::fs::symlink_metadata(p).ok();
                SelectionItem {
                    name: p
                        .file_name()
                        .map(|n| key::nfc(&n.to_string_lossy()))
                        .unwrap_or_default(),
                    size: meta.as_ref().filter(|m| m.is_file()).map_or(0, |m| m.len()),
                    is_dir: meta.as_ref().is_some_and(|m| m.is_dir()),
                }
            })
            .collect();
        let id = uuid::Uuid::new_v4().to_string();
        let mut map = self.items.lock().unwrap();
        map.retain(|_, s| s.created.elapsed() < SELECTION_TTL);
        map.insert(
            id.clone(),
            Stored {
                paths,
                created: Instant::now(),
            },
        );
        Selection {
            selection_id: id,
            items,
        }
    }

    /// 選択 ID に対応するパスを返す。失効・不明な ID はエラー。
    pub fn get(&self, id: &str) -> CoreResult<Vec<PathBuf>> {
        let map = self.items.lock().unwrap();
        match map.get(id) {
            Some(s) if s.created.elapsed() < SELECTION_TTL => Ok(s.paths.clone()),
            _ => Err(CoreError::with_message(
                ErrorCode::NotFound,
                "選択が無効になりました。もう一度選択してください",
            )),
        }
    }

    pub fn remove(&self, id: &str) {
        self.items.lock().unwrap().remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_and_resolves_selections() {
        let dir = crate::store::db::tempfile_guard::TempDir::new().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").unwrap();
        let reg = SelectionRegistry::new();
        let sel = reg.register(vec![file.clone(), dir.path().to_path_buf()]);
        assert_eq!(sel.items[0].name, "a.txt");
        assert_eq!(sel.items[0].size, 5);
        assert!(!sel.items[0].is_dir);
        assert!(sel.items[1].is_dir);
        assert_eq!(reg.get(&sel.selection_id).unwrap()[0], file);
        assert!(reg.get("unknown").is_err());
        reg.remove(&sel.selection_id);
        assert!(reg.get(&sel.selection_id).is_err());
    }
}
