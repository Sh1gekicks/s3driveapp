// Mock S3 data for the UI kit (two buckets, nested prefixes, object versions, cost metrics).
(function () {
  const NOW = new Date(2026, 8, 27, 14, 40);
  const pad = (n) => String(n).padStart(2, '0');
  const fmtDate = (d) => d.getFullYear() + '/' + pad(d.getMonth() + 1) + '/' + pad(d.getDate()) + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes());
  const fmtSize = (b) => {
    if (b == null) return '—';
    if (b < 1000) return b + ' B';
    const u = ['KB', 'MB', 'GB', 'TB']; let i = -1;
    do { b /= 1000; i++; } while (b >= 1000 && i < u.length - 1);
    return (b >= 100 ? Math.round(b) : b.toFixed(1)) + ' ' + u[i];
  };
  let seq = 7;
  const vid = () => { seq = (seq * 48271) % 2147483647; return ('3sL' + seq.toString(36) + 'Qx' + (seq * 7).toString(36) + 'tZ').slice(0, 24); };
  const KB = 1e3, MB = 1e6, GB = 1e9;
  const items = [];
  const d = (m, day, h = 10, min = 0) => new Date(2026, m - 1, day, h, min);
  function folder(bucket, parent, name, modified) {
    items.push({ bucket, parent, name, key: parent + name + '/', folder: true, modified });
  }
  function file(bucket, parent, name, size, modified, created, sc = 'STANDARD', nv = 1) {
    const versions = [];
    for (let i = 0; i < nv; i++) {
      const t = nv === 1 ? modified : new Date(modified.getTime() - (modified - created) * (i / (nv - 1)));
      versions.push({ id: vid(), date: t, size: Math.round(size * (1 - i * 0.035)), latest: i === 0 });
    }
    items.push({ bucket, parent, name, key: parent + name, size, modified, created, sc, versions,
      etag: vid().replace(/[^a-z0-9]/gi, '').toLowerCase().padEnd(32, '0').slice(0, 32),
      contentType: ({ pdf: 'application/pdf', csv: 'text/csv', png: 'image/png', heic: 'image/heic', mov: 'video/quicktime', zip: 'application/zip', gz: 'application/gzip', md: 'text/markdown', rs: 'text/x-rust', json: 'application/json', key: 'application/x-iwork-keynote' })[name.split('.').pop()] || 'application/octet-stream' });
  }
  const T = 'acme-media-tokyo', O = 'acme-backup-osaka';
  ['projects', 'photos', 'backups', 'invoices'].forEach((n, i) => folder(T, '', n, d(9, 26 - i * 3)));
  file(T, '', 'README.md', 2.1 * KB, d(6, 2), d(1, 12), 'STANDARD', 4);
  file(T, '', 'logo.png', 184 * KB, d(3, 18), d(3, 18), 'STANDARD', 1);
  folder(T, 'projects/', '2026', d(9, 27, 14, 32));
  folder(T, 'projects/', '2025', d(1, 8));
  file(T, 'projects/', 'brand-guidelines.pdf', 8.4 * MB, d(4, 11), d(2, 3), 'STANDARD_IA', 3);
  const P = 'projects/2026/';
  file(T, P, 'report-q3.pdf', 4.2 * MB, d(9, 27, 14, 32), d(8, 4, 9, 15), 'STANDARD', 5);
  file(T, P, 'sales-2026.csv', 912 * KB, d(9, 26, 18, 5), d(1, 5), 'INTELLIGENT_TIERING', 12);
  file(T, P, 'keynote-draft.key', 36.8 * MB, d(9, 24, 11, 20), d(9, 2), 'STANDARD', 3);
  file(T, P, 'demo-reel.mov', 842 * MB, d(9, 19, 16, 44), d(9, 19, 16, 44), 'STANDARD', 1);
  file(T, P, 'assets.zip', 128 * MB, d(8, 30, 9, 12), d(7, 1), 'STANDARD_IA', 2);
  file(T, P, 'ingest.rs', 18 * KB, d(9, 22, 21, 3), d(5, 14), 'STANDARD', 7);
  file(T, P, 'cover.png', 2.4 * MB, d(9, 12, 13, 0), d(9, 10), 'STANDARD', 2);
  file(T, 'projects/2025/', 'annual-report-2025.pdf', 12.1 * MB, d(1, 8), d(12, 20), 'GLACIER_IR', 2);
  file(T, 'projects/2025/', 'archive-2025.zip', 2.3 * GB, d(1, 8), d(1, 8), 'GLACIER', 1);
  file(T, 'photos/', 'IMG_2041.heic', 3.1 * MB, d(9, 23, 8, 12), d(9, 23, 8, 12));
  file(T, 'photos/', 'IMG_2042.heic', 2.9 * MB, d(9, 23, 8, 14), d(9, 23, 8, 14));
  file(T, 'photos/', 'IMG_2043.heic', 3.4 * MB, d(9, 23, 8, 20), d(9, 23, 8, 20));
  file(T, 'photos/', 'trip-kyoto.mov', 1.2 * GB, d(8, 16), d(8, 16), 'STANDARD_IA');
  file(T, 'backups/', 'db-2026-09-01.sql.gz', 4.8 * GB, d(9, 1, 3), d(9, 1, 3), 'DEEP_ARCHIVE');
  file(T, 'backups/', 'db-2026-08-01.sql.gz', 4.6 * GB, d(8, 1, 3), d(8, 1, 3), 'DEEP_ARCHIVE');
  file(T, 'invoices/', 'inv-2026-09.pdf', 220 * KB, d(9, 5), d(9, 5), 'GLACIER_IR');
  file(T, 'invoices/', 'inv-2026-08.pdf', 214 * KB, d(8, 5), d(8, 5), 'GLACIER_IR');
  folder(O, '', 'snapshots', d(9, 1));
  file(O, '', 'config.json', 1.2 * KB, d(7, 2), d(2, 2), 'STANDARD', 3);
  file(O, 'snapshots/', 'snap-2026-09.tar.gz', 42 * GB, d(9, 1, 4), d(9, 1, 4), 'DEEP_ARCHIVE');
  file(O, 'snapshots/', 'snap-2026-08.tar.gz', 41.6 * GB, d(8, 1, 4), d(8, 1, 4), 'DEEP_ARCHIVE');

  const PRICE = { STANDARD: 0.025, INTELLIGENT_TIERING: 0.025, STANDARD_IA: 0.0138, ONEZONE_IA: 0.011, GLACIER_IR: 0.005, GLACIER: 0.0045, DEEP_ARCHIVE: 0.002 };
  const daily = (base, n) => Array.from({ length: 30 }, (_, i) => (i < n ? +(base * (0.8 + 0.4 * Math.abs(Math.sin(i * 1.7)))).toFixed(3) : null));
  const buckets = [
    { name: T, region: 'ap-northeast-1', regionLabel: 'アジアパシフィック (東京)', regionShort: '東京', versioning: true, objects: 12408,
      usage: { STANDARD: 180.4, INTELLIGENT_TIERING: 12.6, STANDARD_IA: 29.8, GLACIER_IR: 18.2, GLACIER: 1.4, DEEP_ARCHIVE: 6.2 },
      cost: { requests: 0.42, transfer: 1.14, retrieval: 0.03 }, prevMonth: 6.66, daily: daily(0.257, 27) },
    { name: O, region: 'ap-northeast-3', regionLabel: 'アジアパシフィック (大阪)', regionShort: '大阪', versioning: false, objects: 38,
      usage: { STANDARD: 0.4, DEEP_ARCHIVE: 84.0 }, cost: { requests: 0.01, transfer: 0, retrieval: 0 }, prevMonth: 0.18, daily: daily(0.0068, 27) },
  ];
  const CLASS_INFO = {
    STANDARD: { desc: '頻繁にアクセスするデータ向け', min: null, retrieval: 'ミリ秒' },
    INTELLIGENT_TIERING: { desc: 'アクセス頻度に応じて自動で階層を移動', min: null, retrieval: 'ミリ秒' },
    STANDARD_IA: { desc: 'アクセス頻度は低いが即時取得が必要なデータ', min: '30 日', retrieval: 'ミリ秒' },
    ONEZONE_IA: { desc: '単一 AZ に保存。再作成可能なデータ向け', min: '30 日', retrieval: 'ミリ秒' },
    GLACIER_IR: { desc: '四半期に一度程度のアクセス。即時取得', min: '90 日', retrieval: 'ミリ秒' },
    GLACIER: { desc: '年に数回のアクセス。取り出しに数分〜数時間', min: '90 日', retrieval: '数分〜12 時間', slow: true },
    DEEP_ARCHIVE: { desc: '長期保管用。最も低コスト', min: '180 日', retrieval: '12〜48 時間', slow: true },
  };
  const KIND_LABEL = { folder: 'フォルダ', image: '画像', video: 'ムービー', audio: 'オーディオ', pdf: 'PDF 書類', sheet: 'スプレッドシート', archive: 'アーカイブ', code: 'ソースコード', doc: '書類' };
  window.S3DATA = { NOW, items, buckets, PRICE, CLASS_INFO, KIND_LABEL, fmtDate, fmtSize, vid,
    user: { name: '田中 優希', email: 'yuki.tanaka@gmail.com', initial: '田' } };
})();
