// インスペクタ（03 §5.6）。DS: ui_kits/s3-drive/Inspector.jsx。

import {
  ArchiveRestore,
  CircleAlert,
  Download,
  FolderInput,
  Info,
  Layers,
  RotateCcw,
  Trash2,
} from 'lucide-react';
import type * as React from 'react';
import { useBucketInfo, useFolderSummary, useObjectDetail, useVersions } from '@/app/queries';
import { FileIcon } from '@/components/ds/file-icon';
import { Icon } from '@/components/ds/icon';
import { StorageClassBadge } from '@/components/ds/storage-class-badge';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { IconButton } from '@/components/ui/icon-button';
import { Skeleton, Spinner } from '@/components/ui/misc';
import { SegmentedControl } from '@/components/ui/segmented-control';
import { Tooltip } from '@/components/ui/tooltip';
import { downloadVersion, restoreVersion, runSelectionAction } from '@/features/actions';
import { isArchived, useSelectedEntries } from '@/features/context';
import { isCredentialError } from '@/features/errors';
import { fileKind, KIND_LABEL } from '@/lib/file-kind';
import { formatDate, formatNumber, formatSize, formatSizeDetail } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import {
  type BucketInfo,
  type Connection,
  type Entry,
  type RestoreState,
  toAppError,
  type Versioning,
} from '@/lib/ipc';
import { isArchiveClass } from '@/lib/storage-class';
import { cn } from '@/lib/utils';
import { useNavStore } from '@/stores/nav';
import { useUiStore } from '@/stores/ui';

const t = ja.inspector;

function Header({ name, folder, sub }: { name: string; folder?: boolean; sub: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2.5">
      <FileIcon name={name} folder={folder} size={36} />
      <div className="min-w-0">
        <div className="text-base font-semibold break-all selectable">{name}</div>
        <div className="text-sm text-muted-foreground">{sub}</div>
      </div>
    </div>
  );
}

function Row({
  label,
  children,
  mono,
}: {
  label: React.ReactNode;
  children: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className={cn('m-0 min-w-0 break-all selectable', mono ? 'font-mono text-xs' : 'text-sm')}>
        {children}
      </dd>
    </>
  );
}

function Grid({ children }: { children: React.ReactNode }) {
  return (
    <dl className="m-0 grid grid-cols-[88px_1fr] items-baseline gap-x-2.5 gap-y-2 text-sm">{children}</dl>
  );
}

function versioningLabel(v: Versioning | undefined): string {
  return v ? ja.versioning[v] : ja.common.none;
}

function restoreLabel(r: RestoreState): string {
  switch (r.state) {
    case 'archived':
      return ja.list.restoreNeeded;
    case 'inProgress':
      return ja.list.restoring;
    case 'restored':
      return t.restoredUntil(formatDate(r.expiry));
    default:
      return ja.common.none;
  }
}

const wrap = 'flex flex-col gap-4 p-4';

/** 取得できなかった理由と対処。権限がない場合、理由には必要な IAM アクションが含まれる（05 §5）。 */
function LoadError({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  const err = toAppError(error);
  return (
    <div role="alert" className="flex items-start gap-1.5 text-sm text-destructive">
      <Icon icon={CircleAlert} size={14} className="mt-0.5 shrink-0" />
      <div className="flex min-w-0 flex-col items-start gap-1">
        <span className="break-words selectable">{err.message || ja.list.loadFailed}</span>
        {isCredentialError(err) ? (
          <Button
            variant="link"
            size="sm"
            onClick={() => useUiStore.getState().openDialog({ type: 'credentials' })}
          >
            {ja.menu.credentials}
          </Button>
        ) : (
          <Button variant="link" size="sm" onClick={onRetry}>
            {ja.common.retry}
          </Button>
        )}
      </div>
    </div>
  );
}

function MultiInspector({ items }: { items: Entry[] }) {
  const bytes = items.reduce((s, e) => s + (e.type === 'file' ? e.size : 0), 0);
  return (
    <div className={wrap}>
      <Header name={t.multi(items.length)} sub={t.multiSub(formatSize(bytes))} />
      <div className="flex flex-col gap-2">
        <Button onClick={() => runSelectionAction('download', items)}>
          <Icon icon={Download} size={16} />
          {ja.common.download}
        </Button>
        <Button variant="outline" onClick={() => runSelectionAction('move', items)}>
          <Icon icon={FolderInput} size={16} />
          {ja.menu.move}
        </Button>
        <Button variant="outline" onClick={() => runSelectionAction('storageClass', items)}>
          <Icon icon={Layers} size={16} />
          {ja.menu.storageClass}
        </Button>
        <Button
          variant="ghost"
          className="text-destructive"
          onClick={() => runSelectionAction('delete', items)}
        >
          <Icon icon={Trash2} size={16} />
          {ja.common.delete}
        </Button>
      </div>
    </div>
  );
}

function FolderInspector({
  connection,
  bucket,
  folder,
}: {
  connection: Connection;
  bucket: BucketInfo | undefined;
  folder: Entry | null;
}) {
  const prefix = useNavStore((s) => s.prefix);
  const key = folder ? folder.key : prefix;
  const summary = useFolderSummary(connection.id, key);
  const name = folder
    ? folder.name
    : prefix
      ? (prefix.split('/').filter(Boolean).pop() ?? prefix)
      : connection.bucket;
  const sub = summary.data
    ? t.folderSub(
        `${formatNumber(summary.data.itemCount)}${summary.data.truncated ? '+' : ''}`,
        formatSize(summary.data.totalBytes),
      )
    : summary.isError
      ? ja.common.none
      : ja.common.computing;
  return (
    <div className={wrap}>
      <Header name={name} folder sub={sub} />
      <Grid>
        <Row label={t.bucket}>{connection.bucket}</Row>
        <Row label={t.region}>{connection.regionLabel}</Row>
        <Row label={t.prefix} mono>
          {key || '/'}
        </Row>
        <Row label={t.versioning}>{versioningLabel(bucket?.versioning)}</Row>
      </Grid>
      {folder && !folder.deleted ? (
        <div className="flex gap-2">
          <Button variant="outline" className="flex-1" onClick={() => runSelectionAction('move', [folder])}>
            <Icon icon={FolderInput} size={16} />
            {ja.menu.move}
          </Button>
          <Button
            variant="outline"
            className="flex-1 text-destructive"
            onClick={() => runSelectionAction('delete', [folder])}
          >
            <Icon icon={Trash2} size={16} />
            {ja.common.delete}
          </Button>
        </div>
      ) : null}
    </div>
  );
}

function Details({
  entry,
  query,
}: {
  entry: Entry & { type: 'file' };
  query: ReturnType<typeof useObjectDetail>;
}) {
  const d = query.data;
  const archived = isArchived(entry);
  // HeadObject で求める項目。取得に失敗したらスケルトンのままにせず「—」にし、下に理由を示す
  const pending = (width: string) =>
    query.isError ? ja.common.none : <Skeleton className={cn('h-3', width)} />;
  return (
    <>
      <Grid>
        <Row label={t.size}>
          <span className="tabular-nums">{formatSizeDetail(entry.size)}</span>
        </Row>
        <Row label={t.created}>
          {d ? (
            <span className="inline-flex items-center gap-1 tabular-nums">
              {formatDate(d.created.at)}
              {d.created.source === 'lastModified' ? (
                <Tooltip label={t.createdSameAsModified}>
                  <button
                    type="button"
                    className="inline-flex rounded-xs text-muted-foreground outline-none focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]"
                    aria-label={t.createdSameAsModified}
                  >
                    <Icon icon={Info} size={12} />
                  </button>
                </Tooltip>
              ) : null}
            </span>
          ) : (
            pending('w-28')
          )}
        </Row>
        <Row label={t.modified}>
          <span className="tabular-nums">{formatDate(entry.lastModified)}</span>
        </Row>
        <dt className="text-muted-foreground">{t.storageClass}</dt>
        <dd className="m-0 flex items-center gap-2">
          <StorageClassBadge value={d?.storageClass ?? entry.storageClass} />
          <Button variant="link" size="sm" onClick={() => runSelectionAction('storageClass', [entry])}>
            {ja.common.change}
          </Button>
        </dd>
        <Row label={t.contentType} mono>
          {d ? d.contentType : pending('w-24')}
        </Row>
        <Row label={t.key} mono>
          {entry.key}
        </Row>
        <Row label={t.etag} mono>
          {entry.etag.replaceAll('"', '')}
        </Row>
        <Row label={t.encryption}>
          {d ? (d.kmsKeyId ? `${d.encryption}（${d.kmsKeyId}）` : d.encryption) : pending('w-24')}
        </Row>
        {d?.versionId && d.versionId !== 'null' ? (
          <Row label={t.versionId} mono>
            {d.versionId}
          </Row>
        ) : null}
        {isArchiveClass(entry.storageClass) || entry.restore.state !== 'notArchived' ? (
          <Row label={t.restoreState}>{restoreLabel(d?.restore ?? entry.restore)}</Row>
        ) : null}
      </Grid>
      {query.isError ? <LoadError error={query.error} onRetry={() => void query.refetch()} /> : null}
      {d && Object.keys(d.userMetadata).length > 0 ? (
        <details className="text-sm">
          <summary className="cursor-default text-muted-foreground select-none">{t.userMetadata}</summary>
          <Grid>
            {Object.entries(d.userMetadata).map(([k, v]) => (
              <Row key={k} label={<span className="font-mono text-xs break-all">{k}</span>} mono>
                {v}
              </Row>
            ))}
          </Grid>
        </details>
      ) : null}
      <div className="flex flex-col gap-2">
        {archived ? (
          <Button onClick={() => runSelectionAction('restore', [entry])}>
            <Icon icon={ArchiveRestore} size={16} />
            {ja.menu.restore}
          </Button>
        ) : (
          <Button
            onClick={() => runSelectionAction('download', [entry])}
            disabled={entry.restore.state === 'inProgress'}
          >
            <Icon icon={Download} size={16} />
            {ja.common.download}
          </Button>
        )}
        <div className="flex gap-2">
          <Button variant="outline" className="flex-1" onClick={() => runSelectionAction('move', [entry])}>
            <Icon icon={FolderInput} size={16} />
            {ja.menu.move}
          </Button>
          <Button
            variant="outline"
            className="flex-1 text-destructive"
            onClick={() => runSelectionAction('delete', [entry])}
          >
            <Icon icon={Trash2} size={16} />
            {ja.common.delete}
          </Button>
        </div>
      </div>
    </>
  );
}

function Versions({
  entry,
  bucket,
  query,
}: {
  entry: Entry;
  bucket: BucketInfo | undefined;
  query: ReturnType<typeof useVersions>;
}) {
  const versions = query.data?.pages.flatMap((p) => p.versions) ?? [];
  const disabled = bucket?.versioning === 'disabled';
  return (
    <div className="-mx-2 flex flex-col">
      {disabled ? (
        <div className="px-2 pb-2 text-sm text-muted-foreground">{t.versioningDisabled}</div>
      ) : null}
      {query.isPending && !disabled ? <Skeleton className="mx-2 h-10" /> : null}
      {versions.map((v) => (
        <div
          key={v.versionId}
          className={cn('flex items-center gap-2 rounded-md p-2', v.isLatest && 'bg-muted')}
          data-testid="version-row"
        >
          <div className="flex w-2 justify-center self-stretch">
            <span className={cn('mt-1 size-1.75 rounded-full', v.isLatest ? 'bg-primary' : 'bg-gray-400')} />
          </div>
          <div className="flex min-w-0 flex-1 flex-col gap-0.75">
            <div className="flex items-center gap-1.5 text-sm font-semibold tabular-nums">
              <span className="whitespace-nowrap">{formatDate(v.lastModified)}</span>
              {v.isLatest && !v.isDeleteMarker ? <Badge variant="success">{t.latest}</Badge> : null}
              {v.isDeleteMarker ? <Badge variant="destructive">{ja.list.deleteMarker}</Badge> : null}
            </div>
            <div className="flex min-w-0 gap-1.5 text-xs text-muted-foreground">
              <span className="shrink-0 tabular-nums">{formatSize(v.size)}</span>
              <span aria-hidden="true">·</span>
              <span className="truncate font-mono text-[10.5px] selectable" title={v.versionId}>
                {v.versionId}
              </span>
            </div>
          </div>
          <IconButton
            size="sm"
            icon={RotateCcw}
            label={t.restoreVersion}
            tooltipSide="left"
            disabled={v.isLatest || v.isDeleteMarker}
            onClick={() => void restoreVersion(entry.key, v.versionId, v.lastModified)}
          />
          <IconButton
            size="sm"
            icon={Download}
            label={t.downloadVersion}
            tooltipSide="left"
            disabled={v.isDeleteMarker}
            onClick={() => void downloadVersion(entry.key, v.versionId)}
          />
          <IconButton
            size="sm"
            icon={Trash2}
            label={t.deleteVersion}
            tooltipSide="left"
            disabled={versions.length === 1 && !query.hasNextPage}
            onClick={() =>
              useUiStore.getState().openDialog({
                type: 'deleteVersion',
                key: entry.key,
                versionId: v.versionId,
                date: v.lastModified,
                size: v.size,
                isLatest: v.isLatest,
              })
            }
          />
        </div>
      ))}
      {query.hasNextPage ? (
        <Button variant="link" size="sm" className="self-center" onClick={() => void query.fetchNextPage()}>
          {query.isFetchingNextPage ? <Spinner size={12} /> : null}
          {ja.common.loadMore}
        </Button>
      ) : null}
      {query.isError ? (
        <div className="px-2">
          <LoadError error={query.error} onRetry={() => void query.refetch()} />
        </div>
      ) : null}
    </div>
  );
}

function FileInspector({
  connection,
  entry,
  bucket,
}: {
  connection: Connection;
  entry: Entry & { type: 'file' };
  bucket: BucketInfo | undefined;
}) {
  const tab = useUiStore((s) => s.inspectorTab);
  const detail = useObjectDetail(connection.id, entry.key);
  const versions = useVersions(
    connection.id,
    entry.key,
    tab === 'versions' || bucket?.versioning !== 'disabled',
  );
  const count = versions.data ? versions.data.pages.reduce((s, p) => s + p.versions.length, 0) : null;
  return (
    <div className={wrap}>
      <Header name={entry.name} sub={`${KIND_LABEL[fileKind(entry.name)]} · ${formatSize(entry.size)}`} />
      <SegmentedControl
        block
        aria-label="インスペクタ"
        value={tab}
        onChange={(v) => useUiStore.getState().setInspectorTab(v)}
        options={[
          { value: 'details', label: t.details },
          { value: 'versions', label: t.versions(count) },
        ]}
      />
      {tab === 'details' ? (
        <Details entry={entry} query={detail} />
      ) : (
        <Versions entry={entry} bucket={bucket} query={versions} />
      )}
    </div>
  );
}

function DeletedInspector({ items }: { items: Entry[] }) {
  const [first] = items;
  if (!first) return null;
  return (
    <div className={wrap}>
      <Header
        name={items.length === 1 ? first.name : t.multi(items.length)}
        folder={first.type === 'folder'}
        sub={ja.list.deleteMarker}
      />
      <div className="flex flex-col gap-2">
        <Button onClick={() => runSelectionAction('undelete', items)}>
          <Icon icon={RotateCcw} size={16} />
          {ja.menu.undelete}
        </Button>
        <Button
          variant="ghost"
          className="text-destructive"
          onClick={() => runSelectionAction('purge', items)}
        >
          <Icon icon={Trash2} size={16} />
          {ja.menu.purge}
        </Button>
      </div>
    </div>
  );
}

export function Inspector({ connection }: { connection: Connection }) {
  const items = useSelectedEntries();
  const bucket = useBucketInfo(connection.id).data;
  const [first] = items;
  if (items.length > 0 && items.every((e) => e.deleted)) return <DeletedInspector items={items} />;
  if (items.length > 1) return <MultiInspector items={items} />;
  if (first && first.type === 'file')
    return <FileInspector key={first.key} connection={connection} entry={first} bucket={bucket} />;
  return <FolderInspector connection={connection} bucket={bucket} folder={first ?? null} />;
}
