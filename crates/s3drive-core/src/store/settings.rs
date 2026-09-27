//! 設定ファイル（`settings.json`。06 §2）。変更は即時に保存する。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{CoreError, CoreResult};
use crate::model::{SETTINGS_VERSION, Settings, SettingsFile};

pub struct SettingsStore {
    path: Option<PathBuf>,
    data: Mutex<SettingsFile>,
}

impl std::fmt::Debug for SettingsStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsStore")
            .field("path", &self.path)
            .finish()
    }
}

impl SettingsStore {
    /// ファイルから読み込む。ない場合は既定値。読めない・新しすぎる形式は退避して既定値にする。
    pub fn load(path: &Path) -> CoreResult<Self> {
        let data = match std::fs::read(path) {
            Ok(bytes) => Self::parse_or_backup(path, &bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => SettingsFile::default(),
            Err(e) => return Err(CoreError::local_io(path, &e)),
        };
        Ok(Self {
            path: Some(path.to_path_buf()),
            data: Mutex::new(data),
        })
    }

    /// メモリ上だけの設定（テスト用）。
    pub fn in_memory(data: SettingsFile) -> Self {
        Self {
            path: None,
            data: Mutex::new(data),
        }
    }

    fn parse_or_backup(path: &Path, bytes: &[u8]) -> SettingsFile {
        let backup = || {
            let bak = path.with_extension("json.bak");
            if let Err(e) = std::fs::write(&bak, bytes) {
                log::warn!("設定ファイルを退避できません: {e}");
            }
        };
        let value: serde_json::Value = match serde_json::from_slice(bytes) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("設定ファイルを読めないため既定値に戻します: {e}");
                backup();
                return SettingsFile::default();
            }
        };
        let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if version != SETTINGS_VERSION {
            // 古い（または新しい）形式は退避してから変換する（06 §2）。現在は版 1 のみ。
            backup();
            log::info!("設定ファイルの版 {version} を {SETTINGS_VERSION} に変換します");
        }
        match serde_json::from_value::<SettingsFile>(value) {
            Ok(mut file) => {
                file.version = SETTINGS_VERSION;
                file
            }
            Err(e) => {
                log::warn!("設定ファイルの形式が不正なため既定値に戻します: {e}");
                backup();
                SettingsFile::default()
            }
        }
    }

    pub fn read<R>(&self, f: impl FnOnce(&SettingsFile) -> R) -> R {
        let guard = self.data.lock().unwrap_or_else(|e| e.into_inner());
        f(&guard)
    }

    /// 変更して保存する。保存に失敗した場合は変更を戻す。
    pub fn update<R>(&self, f: impl FnOnce(&mut SettingsFile) -> CoreResult<R>) -> CoreResult<R> {
        let mut guard = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let before = guard.clone();
        let result = f(&mut guard);
        match result {
            Ok(value) => {
                if let Err(e) = self.save(&guard) {
                    *guard = before;
                    return Err(e);
                }
                Ok(value)
            }
            Err(e) => {
                *guard = before;
                Err(e)
            }
        }
    }

    pub fn settings(&self) -> Settings {
        self.read(|f| f.settings.clone())
    }

    /// `patch`（部分的な JSON）を現在の設定に重ねて保存する。
    pub fn patch(&self, patch: serde_json::Value) -> CoreResult<Settings> {
        self.update(|file| {
            let mut value = serde_json::to_value(&file.settings)?;
            merge_json(&mut value, patch);
            let mut settings: Settings = serde_json::from_value(value).map_err(|e| {
                CoreError::new(crate::error::ErrorCode::Internal)
                    .detail(format!("invalid settings: {e}"))
            })?;
            settings.transfer = settings.transfer.clamped();
            file.settings = settings.clone();
            Ok(settings)
        })
    }

    fn save(&self, data: &SettingsFile) -> CoreResult<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| CoreError::local_io(dir, &e))?;
        }
        let json = serde_json::to_vec_pretty(data)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| CoreError::local_io(&tmp, &e))?;
        std::fs::rename(&tmp, path).map_err(|e| CoreError::local_io(path, &e))?;
        Ok(())
    }
}

/// JSON のオブジェクトを再帰的に重ねる（配列・値は置き換え）。
pub fn merge_json(target: &mut serde_json::Value, patch: serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(t), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                merge_json(t.entry(k).or_insert(serde_json::Value::Null), v);
            }
        }
        (t, p) => *t = p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Appearance;
    use crate::store::db::tempfile_guard::TempDir;

    #[test]
    fn patches_nested_values_and_persists() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(&path).unwrap();
        let s = store
            .patch(serde_json::json!({ "general": { "appearance": "dark" }, "transfer": { "maxFiles": 20 } }))
            .unwrap();
        assert_eq!(s.general.appearance, Appearance::Dark);
        assert!(!s.general.show_hidden);
        assert_eq!(s.transfer.max_files, 8);

        let reloaded = SettingsStore::load(&path).unwrap();
        assert_eq!(reloaded.settings().general.appearance, Appearance::Dark);
    }

    #[test]
    fn invalid_patch_is_rejected_without_changes() {
        let store = SettingsStore::in_memory(SettingsFile::default());
        assert!(
            store
                .patch(serde_json::json!({ "general": { "appearance": 3 } }))
                .is_err()
        );
        assert_eq!(store.settings().general.appearance, Appearance::Auto);
    }

    #[test]
    fn broken_files_are_backed_up() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"{ not json").unwrap();
        let store = SettingsStore::load(&path).unwrap();
        assert_eq!(store.settings(), Settings::default());
        assert!(dir.path().join("settings.json.bak").exists());
    }
}
