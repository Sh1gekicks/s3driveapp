//! リージョンコードから日本語名・短縮名への対応表（04 §12.1）。

/// （コード、日本語名、短縮名）。
pub const REGIONS: &[(&str, &str, &str)] = &[
    ("ap-northeast-1", "アジアパシフィック (東京)", "東京"),
    ("ap-northeast-3", "アジアパシフィック (大阪)", "大阪"),
    ("ap-northeast-2", "アジアパシフィック (ソウル)", "ソウル"),
    ("ap-east-1", "アジアパシフィック (香港)", "香港"),
    (
        "ap-southeast-1",
        "アジアパシフィック (シンガポール)",
        "シンガポール",
    ),
    (
        "ap-southeast-2",
        "アジアパシフィック (シドニー)",
        "シドニー",
    ),
    (
        "ap-southeast-3",
        "アジアパシフィック (ジャカルタ)",
        "ジャカルタ",
    ),
    ("ap-south-1", "アジアパシフィック (ムンバイ)", "ムンバイ"),
    ("us-east-1", "米国東部 (バージニア北部)", "バージニア北部"),
    ("us-east-2", "米国東部 (オハイオ)", "オハイオ"),
    (
        "us-west-1",
        "米国西部 (北カリフォルニア)",
        "北カリフォルニア",
    ),
    ("us-west-2", "米国西部 (オレゴン)", "オレゴン"),
    ("ca-central-1", "カナダ (中部)", "カナダ中部"),
    ("eu-west-1", "欧州 (アイルランド)", "アイルランド"),
    ("eu-west-2", "欧州 (ロンドン)", "ロンドン"),
    ("eu-west-3", "欧州 (パリ)", "パリ"),
    ("eu-central-1", "欧州 (フランクフルト)", "フランクフルト"),
    ("eu-north-1", "欧州 (ストックホルム)", "ストックホルム"),
    ("sa-east-1", "南米 (サンパウロ)", "サンパウロ"),
];

/// 日本語名。表にないリージョンはコードをそのまま返す。
pub fn label(code: &str) -> String {
    REGIONS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, l, _)| (*l).to_string())
        .unwrap_or_else(|| code.to_string())
}

/// 短縮名。表にないリージョンはコードをそのまま返す。
pub fn short(code: &str) -> String {
    REGIONS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, _, s)| (*s).to_string())
        .unwrap_or_else(|| code.to_string())
}

/// Cost Explorer の使用タイプに付くリージョン接頭辞（例: `APN1-`）。
pub fn usage_type_prefix(code: &str) -> Option<&'static str> {
    Some(match code {
        "us-east-1" => "USE1",
        "us-east-2" => "USE2",
        "us-west-1" => "USW1",
        "us-west-2" => "USW2",
        "ap-northeast-1" => "APN1",
        "ap-northeast-2" => "APN2",
        "ap-northeast-3" => "APN3",
        "ap-southeast-1" => "APS1",
        "ap-southeast-2" => "APS2",
        "ap-south-1" => "APS3",
        "ap-east-1" => "APE1",
        "eu-west-1" => "EU",
        "eu-west-2" => "EUW2",
        "eu-west-3" => "EUW3",
        "eu-central-1" => "EUC1",
        "eu-north-1" => "EUN1",
        "ca-central-1" => "CAN1",
        "sa-east-1" => "SAE1",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_regions() {
        assert_eq!(label("ap-northeast-1"), "アジアパシフィック (東京)");
        assert_eq!(short("ap-northeast-3"), "大阪");
        assert_eq!(short("xx-unknown-1"), "xx-unknown-1");
    }
}
