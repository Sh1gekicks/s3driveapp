// モックデータ（DS: ui_kits/s3-drive/data.js に合わせる。09 §2.4）。

import type { StorageClass, Versioning } from '../types';

export interface MockVersion {
  id: string;
  date: Date;
  size: number | null;
  deleteMarker: boolean;
  storageClass: StorageClass;
}

export interface MockObject {
  key: string;
  contentType: string;
  etag: string;
  metadata: Record<string, string>;
  /** 新しい順。先頭が最新。 */
  versions: MockVersion[];
  restore?: { state: 'inProgress' } | { state: 'restored'; expiry: string };
}

export interface MockBucket {
  id: string;
  name: string;
  region: string;
  versioning: Versioning;
  encryption: string;
  objectCount: number;
  usageGb: Partial<Record<StorageClass, number>>;
  cost: { requests: number; transfer: number; retrieval: number; prevMonth: number; dailyBase: number };
}

/** 画面の見た目を安定させるための基準日時（DS と同じ 2026/09/27 14:40）。 */
export const NOW = new Date(2026, 8, 27, 14, 40);

const KB = 1e3;
const MB = 1e6;
const GB = 1e9;

let seq = 7;
export function versionId(): string {
  seq = (seq * 48271) % 2147483647;
  return `3sL${seq.toString(36)}Qx${(seq * 7).toString(36)}tZ`.slice(0, 24);
}

const CONTENT_TYPES: Record<string, string> = {
  pdf: 'application/pdf',
  csv: 'text/csv',
  png: 'image/png',
  heic: 'image/heic',
  mov: 'video/quicktime',
  zip: 'application/zip',
  gz: 'application/gzip',
  md: 'text/markdown',
  rs: 'text/x-rust',
  json: 'application/json',
  key: 'application/x-iwork-keynote',
};

export function contentTypeOf(name: string): string {
  return CONTENT_TYPES[name.split('.').pop()?.toLowerCase() ?? ''] ?? 'application/octet-stream';
}

function etag(): string {
  return versionId()
    .replace(/[^a-z0-9]/gi, '')
    .toLowerCase()
    .padEnd(32, '0')
    .slice(0, 32);
}

export function seed(): { buckets: MockBucket[]; objects: Map<string, Map<string, MockObject>> } {
  seq = 7;
  const d = (m: number, day: number, h = 10, min = 0) => new Date(2026, m - 1, day, h, min);
  const T = 'conn-tokyo';
  const O = 'conn-osaka';
  const objects = new Map<string, Map<string, MockObject>>([
    [T, new Map()],
    [O, new Map()],
  ]);
  const folder = (conn: string, key: string, modified: Date) => {
    objects.get(conn)?.set(key, {
      key,
      contentType: 'application/x-directory',
      etag: etag(),
      metadata: {},
      versions: [{ id: versionId(), date: modified, size: 0, deleteMarker: false, storageClass: 'STANDARD' }],
    });
  };
  const file = (
    conn: string,
    key: string,
    size: number,
    modified: Date,
    created: Date,
    sc: StorageClass = 'STANDARD',
    nv = 1,
  ) => {
    const versions: MockVersion[] = [];
    for (let i = 0; i < nv; i++) {
      const t =
        nv === 1
          ? modified
          : new Date(modified.getTime() - (modified.getTime() - created.getTime()) * (i / (nv - 1)));
      versions.push({
        id: versionId(),
        date: t,
        size: Math.round(size * (1 - i * 0.035)),
        deleteMarker: false,
        storageClass: sc,
      });
    }
    const name = key.split('/').pop() ?? key;
    objects
      .get(conn)
      ?.set(key, { key, contentType: contentTypeOf(name), etag: etag(), metadata: {}, versions });
  };

  ['projects', 'photos', 'backups', 'invoices'].forEach((n, i) => {
    folder(T, `${n}/`, d(9, 26 - i * 3));
  });
  file(T, 'README.md', 2.1 * KB, d(6, 2), d(1, 12), 'STANDARD', 4);
  file(T, 'logo.png', 184 * KB, d(3, 18), d(3, 18));
  folder(T, 'projects/2026/', d(9, 27, 14, 32));
  folder(T, 'projects/2025/', d(1, 8));
  file(T, 'projects/brand-guidelines.pdf', 8.4 * MB, d(4, 11), d(2, 3), 'STANDARD_IA', 3);
  const P = 'projects/2026/';
  file(T, `${P}report-q3.pdf`, 4.2 * MB, d(9, 27, 14, 32), d(8, 4, 9, 15), 'STANDARD', 5);
  file(T, `${P}sales-2026.csv`, 912 * KB, d(9, 26, 18, 5), d(1, 5), 'INTELLIGENT_TIERING', 12);
  file(T, `${P}keynote-draft.key`, 36.8 * MB, d(9, 24, 11, 20), d(9, 2), 'STANDARD', 3);
  file(T, `${P}demo-reel.mov`, 842 * MB, d(9, 19, 16, 44), d(9, 19, 16, 44));
  file(T, `${P}assets.zip`, 128 * MB, d(8, 30, 9, 12), d(7, 1), 'STANDARD_IA', 2);
  file(T, `${P}ingest.rs`, 18 * KB, d(9, 22, 21, 3), d(5, 14), 'STANDARD', 7);
  file(T, `${P}cover.png`, 2.4 * MB, d(9, 12, 13, 0), d(9, 10), 'STANDARD', 2);
  file(T, 'projects/2025/annual-report-2025.pdf', 12.1 * MB, d(1, 8), d(12, 20), 'GLACIER_IR', 2);
  file(T, 'projects/2025/archive-2025.zip', 2.3 * GB, d(1, 8), d(1, 8), 'GLACIER');
  file(T, 'photos/IMG_2041.heic', 3.1 * MB, d(9, 23, 8, 12), d(9, 23, 8, 12));
  file(T, 'photos/IMG_2042.heic', 2.9 * MB, d(9, 23, 8, 14), d(9, 23, 8, 14));
  file(T, 'photos/IMG_2043.heic', 3.4 * MB, d(9, 23, 8, 20), d(9, 23, 8, 20));
  file(T, 'photos/trip-kyoto.mov', 1.2 * GB, d(8, 16), d(8, 16), 'STANDARD_IA');
  file(T, 'backups/db-2026-09-01.sql.gz', 4.8 * GB, d(9, 1, 3), d(9, 1, 3), 'DEEP_ARCHIVE');
  file(T, 'backups/db-2026-08-01.sql.gz', 4.6 * GB, d(8, 1, 3), d(8, 1, 3), 'DEEP_ARCHIVE');
  file(T, 'invoices/inv-2026-09.pdf', 220 * KB, d(9, 5), d(9, 5), 'GLACIER_IR');
  file(T, 'invoices/inv-2026-08.pdf', 214 * KB, d(8, 5), d(8, 5), 'GLACIER_IR');
  folder(O, 'snapshots/', d(9, 1));
  file(O, 'config.json', 1.2 * KB, d(7, 2), d(2, 2), 'STANDARD', 3);
  file(O, 'snapshots/snap-2026-09.tar.gz', 42 * GB, d(9, 1, 4), d(9, 1, 4), 'DEEP_ARCHIVE');
  file(O, 'snapshots/snap-2026-08.tar.gz', 41.6 * GB, d(8, 1, 4), d(8, 1, 4), 'DEEP_ARCHIVE');

  const buckets: MockBucket[] = [
    {
      id: T,
      name: 'acme-media-tokyo',
      region: 'ap-northeast-1',
      versioning: 'enabled',
      encryption: 'SSE-S3 (AES-256)',
      objectCount: 12408,
      usageGb: {
        STANDARD: 180.4,
        INTELLIGENT_TIERING: 12.6,
        STANDARD_IA: 29.8,
        GLACIER_IR: 18.2,
        GLACIER: 1.4,
        DEEP_ARCHIVE: 6.2,
      },
      cost: { requests: 0.42, transfer: 1.14, retrieval: 0.03, prevMonth: 6.66, dailyBase: 0.257 },
    },
    {
      id: O,
      name: 'acme-backup-osaka',
      region: 'ap-northeast-3',
      versioning: 'disabled',
      encryption: 'SSE-S3 (AES-256)',
      objectCount: 38,
      usageGb: { STANDARD: 0.4, DEEP_ARCHIVE: 84.0 },
      cost: { requests: 0.01, transfer: 0, retrieval: 0, prevMonth: 0.18, dailyBase: 0.0068 },
    },
  ];
  return { buckets, objects };
}

/** DS の単価（USD / GB・月）。 */
export const PRICE: Partial<Record<StorageClass, number>> = {
  STANDARD: 0.025,
  INTELLIGENT_TIERING: 0.025,
  STANDARD_IA: 0.0138,
  ONEZONE_IA: 0.011,
  GLACIER_IR: 0.005,
  GLACIER: 0.0045,
  DEEP_ARCHIVE: 0.002,
};
