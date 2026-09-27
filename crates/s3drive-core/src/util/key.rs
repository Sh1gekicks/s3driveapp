//! キーとプレフィックスの正規化・検証・分解（04 §4.4、04 §10.3）。

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use unicode_normalization::UnicodeNormalization;

use crate::error::{CoreError, CoreResult, ErrorCode};

/// S3 のキーの上限（UTF-8 のバイト数）。
pub const MAX_KEY_BYTES: usize = 1024;

/// 表示名・キーの NFC 正規化（macOS の NFD のファイル名に備える）。
pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// 検索用の正規化（NFKC + 小文字化）。全角英数字と半角英数字を同一視する。
pub fn normalize_for_search(s: &str) -> String {
    s.nfkc().collect::<String>().to_lowercase()
}

pub fn has_control_chars(s: &str) -> bool {
    s.chars().any(char::is_control)
}

/// フォルダ名・ファイル名の検証（DLG-01、DLG-10）。
pub fn validate_name(name: &str) -> CoreResult<()> {
    let invalid = |msg: &str| Err(CoreError::with_message(ErrorCode::InvalidName, msg));
    if name.trim().is_empty() {
        return invalid("名前を入力してください");
    }
    if name.contains('/') {
        return invalid("名前に「/」は使用できません");
    }
    if name == "." || name == ".." {
        return invalid("この名前は使用できません");
    }
    if has_control_chars(name) {
        return invalid("この名前は使用できません");
    }
    Ok(())
}

/// キー全体の検証（1,024 バイト以内、制御文字なし）。
pub fn validate_key(key: &str) -> CoreResult<()> {
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(CoreError::with_message(
            ErrorCode::InvalidName,
            "名前が長すぎます",
        ));
    }
    if has_control_chars(key) {
        return Err(CoreError::new(ErrorCode::InvalidName));
    }
    Ok(())
}

/// プレフィックスの検証（空＝ルート、または `/` で終わる）。
pub fn validate_prefix(prefix: &str) -> CoreResult<()> {
    if prefix.is_empty() {
        return Ok(());
    }
    if !prefix.ends_with('/') || prefix.starts_with('/') || prefix.len() > MAX_KEY_BYTES {
        return Err(CoreError::new(ErrorCode::InvalidName).detail("invalid prefix"));
    }
    if has_control_chars(prefix) {
        return Err(CoreError::new(ErrorCode::InvalidName));
    }
    Ok(())
}

pub fn is_folder_key(key: &str) -> bool {
    key.ends_with('/')
}

/// 親プレフィックス（`a/b/c.txt` → `a/b/`、`a/b/` → `a/`、`c.txt` → ``）。
pub fn parent_prefix(key: &str) -> &str {
    let trimmed = key.strip_suffix('/').unwrap_or(key);
    match trimmed.rfind('/') {
        Some(i) => &key[..=i],
        None => "",
    }
}

/// 最後の階層の名前（`a/b/` → `b`、`a/c.txt` → `c.txt`）。
pub fn base_name(key: &str) -> &str {
    let trimmed = key.strip_suffix('/').unwrap_or(key);
    match trimmed.rfind('/') {
        Some(i) => &trimmed[i + 1..],
        None => trimmed,
    }
}

/// 小文字の拡張子（先頭の `.` を除く）。拡張子がない、または隠しファイル名だけの場合は `None`。
pub fn extension(name: &str) -> Option<String> {
    let dot = name.rfind('.')?;
    if dot == 0 || dot + 1 == name.len() {
        return None;
    }
    Some(name[dot + 1..].to_lowercase())
}

/// 名前を「本体」と「拡張子（`.` を含む）」に分ける。
pub fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    }
}

pub fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// 「name (n).ext」形式の名前（DLG-08、04 §5.1）。
pub fn numbered_name(name: &str, n: u32) -> String {
    let (stem, ext) = split_ext(name);
    format!("{stem} ({n}){ext}")
}

/// `exists` が偽になるまで連番を増やした名前を返す。
pub fn unique_name(name: &str, mut exists: impl FnMut(&str) -> bool) -> String {
    if !exists(name) {
        return name.to_string();
    }
    (1..)
        .map(|n| numbered_name(name, n))
        .find(|candidate| !exists(candidate))
        .expect("infinite iterator")
}

/// `EncodingType=url` で返されたキーを復元する（S3 は空白を `+` にする）。
pub fn url_decode_key(encoded: &str) -> String {
    let plus_as_space = encoded.replace('+', " ");
    percent_encoding::percent_decode_str(&plus_as_space)
        .decode_utf8_lossy()
        .into_owned()
}

/// CopySource に使う文字（`/` 以外の予約文字をエンコードする）。
const COPY_SOURCE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// `CopySource`（`{バケット}/{キー}?versionId={ID}`）を作る。
pub fn copy_source(bucket: &str, key: &str, version_id: Option<&str>) -> String {
    let encoded = utf8_percent_encode(key, COPY_SOURCE);
    match version_id {
        Some(v) => format!(
            "{bucket}/{encoded}?versionId={}",
            utf8_percent_encode(v, NON_ALPHANUMERIC)
        ),
        None => format!("{bucket}/{encoded}"),
    }
}

/// 移動先のキー: 移動先プレフィックス＋（元のキーから移動元の親プレフィックスを除いた部分）（04 §7.3）。
pub fn move_destination(key: &str, source_parent: &str, dest_prefix: &str) -> String {
    let rest = key.strip_prefix(source_parent).unwrap_or(key);
    format!("{dest_prefix}{rest}")
}

/// `key` が `folder`（`/` で終わるプレフィックス）自身またはその配下か。
pub fn is_within(key: &str, folder: &str) -> bool {
    is_folder_key(folder) && key.starts_with(folder)
}

/// アクセスキー ID の伏せ字（先頭 4 文字と末尾 4 文字以外を伏せる。01 §7.3）。
pub fn mask_access_key_id(id: &str) -> String {
    let chars: Vec<char> = id.chars().collect();
    if chars.len() <= 8 {
        return "*".repeat(chars.len());
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}{}{tail}", "*".repeat(chars.len() - 8))
}

/// バケット名の検証（3〜63 文字、英小文字・数字・`.`・`-`、先頭と末尾は英数字）。
pub fn is_valid_bucket_name(name: &str) -> bool {
    let len = name.len();
    if !(3..=63).contains(&len) {
        return false;
    }
    let bytes = name.as_bytes();
    let alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    alnum(bytes[0])
        && alnum(bytes[len - 1])
        && bytes.iter().all(|&b| alnum(b) || b == b'.' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_composes_decomposed_names() {
        let nfd = "\u{30CF}\u{309A}\u{30F3}"; // ハ + 半濁点 + ン
        assert_eq!(nfc(nfd), "パン");
        assert_ne!(nfd, "パン");
    }

    #[test]
    fn search_normalization_folds_width_and_case() {
        assert_eq!(normalize_for_search("ＲｅＰｏｒｔ"), "report");
        assert_eq!(normalize_for_search("Report-Q3.PDF"), "report-q3.pdf");
        // ひらがなとカタカナは区別する
        assert_ne!(
            normalize_for_search("れぽーと"),
            normalize_for_search("レポート")
        );
    }

    #[test]
    fn validates_names() {
        assert!(validate_name("新規フォルダ").is_ok());
        for bad in ["", "  ", "a/b", ".", "..", "a\u{0007}"] {
            assert!(validate_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn validates_key_length_in_bytes() {
        assert!(validate_key(&"a".repeat(1024)).is_ok());
        assert!(validate_key(&"a".repeat(1025)).is_err());
        // 「あ」は UTF-8 で 3 バイト
        assert!(validate_key(&"あ".repeat(341)).is_ok());
        assert!(validate_key(&"あ".repeat(342)).is_err());
        assert!(validate_key("a\nb").is_err());
    }

    #[test]
    fn validates_prefixes() {
        assert!(validate_prefix("").is_ok());
        assert!(validate_prefix("a/b/").is_ok());
        assert!(validate_prefix("a/b").is_err());
        assert!(validate_prefix("/a/").is_err());
    }

    #[test]
    fn splits_keys() {
        assert_eq!(parent_prefix("a/b/c.txt"), "a/b/");
        assert_eq!(parent_prefix("a/b/"), "a/");
        assert_eq!(parent_prefix("c.txt"), "");
        assert_eq!(parent_prefix("a/"), "");
        assert_eq!(base_name("a/b/"), "b");
        assert_eq!(base_name("a/c.txt"), "c.txt");
        assert_eq!(base_name("c.txt"), "c.txt");
    }

    #[test]
    fn extracts_extensions() {
        assert_eq!(extension("report.PDF").as_deref(), Some("pdf"));
        assert_eq!(extension("db.sql.gz").as_deref(), Some("gz"));
        assert_eq!(extension(".bashrc"), None);
        assert_eq!(extension("README"), None);
        assert_eq!(extension("dot."), None);
    }

    #[test]
    fn numbers_names_like_browsers() {
        assert_eq!(numbered_name("report.pdf", 1), "report (1).pdf");
        assert_eq!(numbered_name("README", 2), "README (2)");
        assert_eq!(numbered_name(".env", 1), ".env (1)");
        let taken = ["a.txt", "a (1).txt"];
        assert_eq!(unique_name("a.txt", |n| taken.contains(&n)), "a (2).txt");
        assert_eq!(unique_name("b.txt", |n| taken.contains(&n)), "b.txt");
    }

    #[test]
    fn decodes_url_encoded_keys() {
        assert_eq!(url_decode_key("my+file%2B1.txt"), "my file+1.txt");
        assert_eq!(url_decode_key("%E6%97%A5%E6%9C%AC/a%20b"), "日本/a b");
    }

    #[test]
    fn builds_copy_source() {
        assert_eq!(
            copy_source("b", "a b/日.txt", None),
            "b/a%20b/%E6%97%A5.txt"
        );
        assert_eq!(copy_source("b", "x+y", Some("v1")), "b/x%2By?versionId=v1");
    }

    #[test]
    fn computes_move_destination() {
        assert_eq!(move_destination("a/b/c.txt", "a/", "x/"), "x/b/c.txt");
        assert_eq!(move_destination("c.txt", "", "x/"), "x/c.txt");
        assert_eq!(move_destination("a/c.txt", "a/", ""), "c.txt");
    }

    #[test]
    fn checks_folder_containment() {
        assert!(is_within("a/b/", "a/"));
        assert!(is_within("a/", "a/"));
        assert!(!is_within("ab/", "a/"));
        assert!(!is_within("a/b", "a"));
    }

    #[test]
    fn masks_access_key_ids() {
        assert_eq!(
            mask_access_key_id("AKIA4Z7XEXAMPLE7Q2LM"),
            "AKIA************Q2LM"
        );
        assert_eq!(mask_access_key_id("short"), "*****");
    }

    #[test]
    fn validates_bucket_names() {
        assert!(is_valid_bucket_name("acme-media-tokyo"));
        assert!(is_valid_bucket_name("a.b-c"));
        assert!(!is_valid_bucket_name("ab"));
        assert!(!is_valid_bucket_name("Acme"));
        assert!(!is_valid_bucket_name("-acme"));
        assert!(!is_valid_bucket_name("acme-"));
        assert!(!is_valid_bucket_name(&"a".repeat(64)));
    }
}
