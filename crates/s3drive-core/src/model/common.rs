use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 接続 ID（UUID）。
pub type ConnectionId = String;
/// ジョブ ID（UUID）。
pub type JobId = String;
/// RFC 3339（UTC）の日時文字列。
pub type Timestamp = String;

/// 変更先として扱う 7 クラスと、それ以外（`OTHER`）（04 §8.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum StorageClass {
    Standard,
    IntelligentTiering,
    StandardIa,
    OnezoneIa,
    GlacierIr,
    Glacier,
    DeepArchive,
    Other,
}

impl StorageClass {
    pub const SELECTABLE: [StorageClass; 7] = [
        Self::Standard,
        Self::IntelligentTiering,
        Self::StandardIa,
        Self::OnezoneIa,
        Self::GlacierIr,
        Self::Glacier,
        Self::DeepArchive,
    ];

    /// S3 の API が返すクラス名から変換する。省略時は STANDARD（04 §9.1）。
    pub fn from_s3(value: Option<&str>) -> Self {
        match value.unwrap_or("STANDARD") {
            "STANDARD" => Self::Standard,
            "INTELLIGENT_TIERING" => Self::IntelligentTiering,
            "STANDARD_IA" => Self::StandardIa,
            "ONEZONE_IA" => Self::OnezoneIa,
            "GLACIER_IR" => Self::GlacierIr,
            "GLACIER" => Self::Glacier,
            "DEEP_ARCHIVE" => Self::DeepArchive,
            _ => Self::Other,
        }
    }

    pub fn as_s3(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::IntelligentTiering => "INTELLIGENT_TIERING",
            Self::StandardIa => "STANDARD_IA",
            Self::OnezoneIa => "ONEZONE_IA",
            Self::GlacierIr => "GLACIER_IR",
            Self::Glacier => "GLACIER",
            Self::DeepArchive => "DEEP_ARCHIVE",
            Self::Other => "OTHER",
        }
    }

    /// 取り出し（RestoreObject）が必要になりうるクラス。
    pub fn is_archive(self) -> bool {
        matches!(self, Self::Glacier | Self::DeepArchive)
    }

    /// 正式名（DS の STORAGE_CLASSES の label）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::IntelligentTiering => "Intelligent-Tiering",
            Self::StandardIa => "Standard-IA",
            Self::OnezoneIa => "One Zone-IA",
            Self::GlacierIr => "Glacier Instant Retrieval",
            Self::Glacier => "Glacier Flexible Retrieval",
            Self::DeepArchive => "Glacier Deep Archive",
            Self::Other => "その他",
        }
    }

    pub fn to_sdk(self) -> aws_sdk_s3::types::StorageClass {
        aws_sdk_s3::types::StorageClass::from(self.as_s3())
    }
}

/// バケットのバージョニング状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Versioning {
    Enabled,
    Suspended,
    Disabled,
    Unknown,
}

impl Versioning {
    /// バージョン履歴を持ちうる（有効・一時停止）。
    pub fn has_history(self) -> bool {
        matches!(self, Self::Enabled | Self::Suspended)
    }
}

/// ファイル種別（02 §8.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum FileKind {
    Image,
    Video,
    Audio,
    Pdf,
    Sheet,
    Archive,
    Code,
    Doc,
}

impl FileKind {
    /// 種別に対応する拡張子（`doc` は「上記以外」なので空）。
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Image => &["jpg", "jpeg", "png", "gif", "heic", "webp", "svg"],
            Self::Video => &["mp4", "mov"],
            Self::Audio => &["mp3", "wav", "m4a"],
            Self::Pdf => &["pdf"],
            Self::Sheet => &["csv", "xlsx", "numbers"],
            Self::Archive => &["zip", "gz", "tar"],
            Self::Code => &["js", "ts", "json", "py", "rs", "html"],
            Self::Doc => &[],
        }
    }

    pub fn all_known_extensions() -> Vec<&'static str> {
        [
            Self::Image,
            Self::Video,
            Self::Audio,
            Self::Pdf,
            Self::Sheet,
            Self::Archive,
            Self::Code,
        ]
        .iter()
        .flat_map(|k| k.extensions().iter().copied())
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_class_round_trips_s3_names() {
        for class in StorageClass::SELECTABLE {
            assert_eq!(StorageClass::from_s3(Some(class.as_s3())), class);
        }
        assert_eq!(StorageClass::from_s3(None), StorageClass::Standard);
        assert_eq!(
            StorageClass::from_s3(Some("REDUCED_REDUNDANCY")),
            StorageClass::Other
        );
    }

    #[test]
    fn storage_class_serializes_like_the_s3_api() {
        assert_eq!(
            serde_json::to_string(&StorageClass::OnezoneIa).unwrap(),
            "\"ONEZONE_IA\""
        );
        assert_eq!(
            serde_json::to_string(&StorageClass::GlacierIr).unwrap(),
            "\"GLACIER_IR\""
        );
    }
}
