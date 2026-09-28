//! 検索のベンチマーク（09 §5）。100 万件のインデックスに対し、300 ms 以内に結果を返すことを確かめる（README §8）。
//!
//!   cargo bench -p s3drive-core --features bench --bench search
//!
//! インデックスの作成に数十秒かかる。件数は `S3DRIVE_BENCH_OBJECTS` で変えられる（既定 1,000,000）。

use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use s3drive_core::model::{DateFilter, FileKind, SearchQuery, SizeFilter, StorageClass};
use s3drive_core::search::IndexedObject;
use s3drive_core::search::bench_support::{open, query, seed};

const CONNECTION: &str = "bench";
const EXTENSIONS: [&str; 8] = ["pdf", "csv", "png", "heic", "mov", "zip", "md", "rs"];
const CLASSES: [StorageClass; 4] = [
    StorageClass::Standard,
    StorageClass::StandardIa,
    StorageClass::GlacierIr,
    StorageClass::DeepArchive,
];

/// 実際のバケットに近い形のキー（3 階層のフォルダ、日本語名を含む）。
fn objects(count: usize) -> Vec<IndexedObject> {
    (0..count)
        .map(|i| {
            let ext = EXTENSIONS[i % EXTENSIONS.len()];
            let name = if i % 7 == 0 {
                format!("資料-{i:07}.{ext}")
            } else {
                format!("report-{i:07}.{ext}")
            };
            IndexedObject {
                key: format!("projects/{}/team-{}/{name}", 2000 + i % 27, i % 100),
                size: (i as u64 * 7_919) % 500_000_000,
                last_modified: format!("2026-{:02}-{:02}T00:00:00Z", 1 + i % 9, 1 + i % 28),
                etag: None,
                storage_class: CLASSES[i % CLASSES.len()],
            }
        })
        .collect()
}

fn bench(c: &mut Criterion) {
    let count: usize = std::env::var("S3DRIVE_BENCH_OBJECTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    let dir = std::env::temp_dir().join(format!("s3drive-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut conn = open(&dir.join("s3drive.db"));
    seed(&mut conn, CONNECTION, &objects(count));

    let mut group = c.benchmark_group(format!("search {count} objects"));
    group
        .sample_size(20)
        .measurement_time(Duration::from_secs(10));
    let cases: Vec<(&str, SearchQuery)> = vec![
        ("name (FTS5 trigram)", SearchQuery::text("report-00123")),
        ("name (2 chars)", SearchQuery::text("資料")),
        ("name + ext", {
            let mut q = SearchQuery::text("report");
            q.ext = Some("pdf".into());
            q
        }),
        ("kind + size + date", {
            let mut q = SearchQuery::text("");
            q.kind = Some(FileKind::Image);
            q.size = Some(SizeFilter::Gt100);
            q.date = Some(DateFilter::Year);
            q
        }),
        ("storage class", {
            let mut q = SearchQuery::text("");
            q.storage_class = Some(StorageClass::DeepArchive);
            q
        }),
    ];
    for (name, q) in &cases {
        group.bench_function(*name, |b| b.iter(|| query(&conn, CONNECTION, q)));
    }
    group.finish();
    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}

criterion_group!(benches, bench);
criterion_main!(benches);
