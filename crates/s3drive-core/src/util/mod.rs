pub mod key;
pub mod region;
pub mod time;

/// 名前の自然順の比較。数字の並びは数値として比べる（「file2」が「file10」より前）。
///
/// 一覧の並べ替え（`Intl.Collator('ja', { numeric: true })`。04 §3.4）に合わせ、検索結果の並び順にも使う
/// （04 §10.2）。検索インデックスの `name_norm` は小文字化済みのため、大文字と小文字は区別しない。
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    // UTF-8 のバイト順はコードポイント順と同じで、ASCII の数字は複数バイトの文字の途中に現れないため、
    // バイト単位で比べる（検索結果の並べ替えで何度も呼ばれるため、割り当てをしない）
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    let digits_end = |s: &[u8], mut k: usize| {
        while k < s.len() && s[k].is_ascii_digit() {
            k += 1;
        }
        k
    };
    fn trim_zeros(d: &[u8]) -> &[u8] {
        let first = d.iter().position(|&c| c != b'0').unwrap_or(d.len());
        &d[first..]
    }
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (ei, ej) = (digits_end(a, i), digits_end(b, j));
            let (da, db) = (&a[i..ei], &b[j..ej]);
            let (ta, tb) = (trim_zeros(da), trim_zeros(db));
            let ord = ta
                .len()
                .cmp(&tb.len())
                .then_with(|| ta.cmp(tb))
                .then_with(|| da.len().cmp(&db.len()));
            if ord != Ordering::Equal {
                return ord;
            }
            (i, j) = (ei, ej);
        } else {
            if a[i] != b[j] {
                return a[i].cmp(&b[j]);
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

/// 指数バックオフの待ち時間（初回 1 秒、上限 30 秒、ジッターあり。01 §7.2）。
pub fn backoff_delay(attempt: u32) -> std::time::Duration {
    use rand::Rng;
    let base_ms = 1000u64.saturating_mul(1u64 << attempt.min(5));
    let capped = base_ms.min(30_000);
    let jitter = rand::rng().random_range(0..=capped / 2);
    std::time::Duration::from_millis(capped / 2 + jitter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numbers_in_names_as_numbers() {
        use std::cmp::Ordering::*;
        assert_eq!(natural_cmp("file2", "file10"), Less);
        assert_eq!(natural_cmp("file10", "file2"), Greater);
        assert_eq!(natural_cmp("img_0010.jpg", "img_9.jpg"), Greater);
        assert_eq!(natural_cmp("a", "a1"), Less);
        assert_eq!(natural_cmp("report", "report"), Equal);
        assert_eq!(natural_cmp("b.txt", "a.txt"), Greater);
        let mut names = vec!["file10", "file1", "file2", "file02", "資料"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["file1", "file2", "file02", "file10", "資料"]);
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        for attempt in 0..10 {
            let d = backoff_delay(attempt).as_millis() as u64;
            let cap = (1000u64 << attempt.min(5)).min(30_000);
            assert!(d >= cap / 2 && d <= cap, "attempt {attempt}: {d}");
        }
    }
}
