//! マルチパート転送の計算（04 §4.2、04 §5.2）。

pub const MIB: u64 = 1024 * 1024;
/// 設定・画面の MB（10 進。02 §9.2）。
pub const MB: u64 = 1_000_000;
/// パートサイズの基本値。
pub const DEFAULT_PART_SIZE: u64 = 8 * MIB;
/// S3 のパート数の上限。
pub const MAX_PARTS: u64 = 10_000;
/// S3 のパートサイズの上限（5 GiB）。
pub const MAX_PART_SIZE: u64 = 5 * 1024 * MIB;
/// これ以上のファイルは範囲指定の GetObject を並列に行う（64 MB）。
pub const RANGED_DOWNLOAD_THRESHOLD: u64 = 64 * MB;
/// 範囲指定のダウンロードの大きさ。
pub const DOWNLOAD_RANGE_SIZE: u64 = 16 * MIB;
/// これ以下のパートはメモリに読み込んで送り、送信バイト数を細かく数える。
pub const IN_MEMORY_PART_LIMIT: u64 = 64 * MIB;

/// パートサイズ: 8 MiB を基本とし、パート数が 10,000 を超える場合は「ファイルサイズ ÷ 10,000」を
/// 1 MiB 単位で切り上げた値にする（最大 5 GiB）。
pub fn part_size(file_size: u64) -> u64 {
    if file_size.div_ceil(DEFAULT_PART_SIZE) <= MAX_PARTS {
        return DEFAULT_PART_SIZE;
    }
    let raw = file_size.div_ceil(MAX_PARTS);
    (raw.div_ceil(MIB) * MIB).min(MAX_PART_SIZE)
}

/// マルチパートでアップロードするか（04 §4.2）。設定の境界（MB）以上のファイルをマルチパートにする。
pub fn uses_multipart(file_size: u64, threshold_mb: u32) -> bool {
    file_size >= threshold_mb as u64 * MB
}

/// パートの範囲（開始位置、長さ）。
pub fn part_ranges(file_size: u64, part_size: u64) -> Vec<(u64, u64)> {
    if file_size == 0 {
        return vec![(0, 0)];
    }
    (0..file_size.div_ceil(part_size))
        .map(|i| {
            let start = i * part_size;
            (start, part_size.min(file_size - start))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1_000_000_000;
    const TB: u64 = 1_000 * GB;

    #[test]
    fn uses_8_mib_parts_for_ordinary_files() {
        assert_eq!(part_size(16 * 1_000_000), DEFAULT_PART_SIZE);
        // 8 MiB × 10,000 = 約 83.9 GB まではそのまま
        assert_eq!(part_size(DEFAULT_PART_SIZE * MAX_PARTS), DEFAULT_PART_SIZE);
    }

    #[test]
    fn grows_parts_to_stay_within_10000() {
        let size = DEFAULT_PART_SIZE * MAX_PARTS + 1;
        let p = part_size(size);
        assert_eq!(p % MIB, 0);
        assert!(size.div_ceil(p) <= MAX_PARTS);
        let p = part_size(TB);
        assert_eq!(p, 96 * MIB);
        assert!(TB.div_ceil(p) <= MAX_PARTS);
    }

    #[test]
    fn handles_the_50_tb_maximum() {
        let size = 50 * TB;
        let p = part_size(size);
        assert!(p <= MAX_PART_SIZE);
        assert!(size.div_ceil(p) <= MAX_PARTS);
    }

    #[test]
    fn multipart_threshold_is_in_decimal_megabytes() {
        // 設定の 16 MB は 16,000,000 バイト（画面の表記と同じ 10 進）
        assert!(!uses_multipart(16 * 1_000_000 - 1, 16));
        assert!(uses_multipart(16 * 1_000_000, 16));
        assert!(!uses_multipart(7_999_999, 8));
        assert!(uses_multipart(64 * 1_000_000, 64));
    }

    #[test]
    fn splits_into_ranges() {
        assert_eq!(part_ranges(10, 4), vec![(0, 4), (4, 4), (8, 2)]);
        assert_eq!(part_ranges(8, 4), vec![(0, 4), (4, 4)]);
        assert_eq!(part_ranges(0, 4), vec![(0, 0)]);
    }
}
