// 項目の操作のダイアログ（DLG-01〜04、07〜10）。DS: ui_kits/s3-drive/Dialogs.jsx。

import {
  ArchiveRestore,
  ChevronRight,
  Clock,
  CopyPlus,
  FolderInput,
  FolderPlus,
  Layers,
  Pencil,
  Trash2,
} from 'lucide-react';
import * as React from 'react';
import { useFolderChildren, useFolderSummary, usePricing } from '@/app/queries';
import { FileIcon } from '@/components/ds/file-icon';
import { Icon } from '@/components/ds/icon';
import { StorageClassBadge } from '@/components/ds/storage-class-badge';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Dialog } from '@/components/ui/dialog';
import { Field } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/misc';
import { NativeSelect } from '@/components/ui/native-select';
import {
  baseName,
  changeStorageClass,
  createFolder,
  deleteEntries,
  deleteVersion,
  isVersioned,
  move,
  rename,
  requestRestore,
} from '@/features/actions';
import { isArchived, visibleEntries } from '@/features/context';
import { formatDate, formatSize, formatUsd } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import type {
  ConflictDecision,
  Connection,
  Entry,
  RemoteConflict,
  RestoreTier,
  StorageClass,
  UploadConflict,
} from '@/lib/ipc';
import { classInfo, classLabel, SELECTABLE_CLASSES, STORAGE_CLASSES } from '@/lib/storage-class';
import { cn } from '@/lib/utils';
import { validateName } from '@/lib/validation';
import { parentPrefix, useNavStore } from '@/stores/nav';
import type { ConflictResolver } from '@/stores/ui';

const d = ja.dialog;

interface Base {
  onClose: () => void;
}

function Cancel({ onClick }: { onClick: () => void }) {
  return (
    <Button variant="outline" onClick={onClick}>
      {ja.common.cancel}
    </Button>
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return <p className="m-0 text-sm text-pretty text-muted-foreground">{children}</p>;
}

// ---- DLG-01 新規フォルダ ----------------------------------------------------------------

export function NewFolderDialog({ onClose }: Base) {
  const prefix = useNavStore((s) => s.prefix);
  const [name, setName] = React.useState<string>(d.newFolder.defaultName);
  const [busy, setBusy] = React.useState(false);
  const siblings = React.useMemo(
    () =>
      visibleEntries()
        .filter((e) => e.type === 'folder')
        .map((e) => e.name),
    [],
  );
  const error = validateName(name, prefix, siblings);
  const touched = name !== d.newFolder.defaultName;
  const submit = async () => {
    if (error || busy) return;
    setBusy(true);
    if (await createFolder(name.trim())) onClose();
    setBusy(false);
  };
  return (
    <Dialog
      open
      onClose={onClose}
      icon={FolderPlus}
      title={d.newFolder.title}
      description={d.newFolder.description}
      onSubmit={submit}
      footer={
        <>
          <Cancel onClick={onClose} />
          <Button type="submit" disabled={Boolean(error) || busy}>
            {busy ? <Spinner /> : null}
            {ja.common.create}
          </Button>
        </>
      }
    >
      <Field
        label={d.newFolder.name}
        error={touched || error === d.newFolder.duplicate ? (error ?? undefined) : undefined}
      >
        {(p) => (
          <Input
            {...p}
            autoFocus
            value={name}
            onFocus={(e) => e.currentTarget.select()}
            onChange={(e) => setName(e.target.value)}
          />
        )}
      </Field>
    </Dialog>
  );
}

// ---- DLG-10 名前を変更 -------------------------------------------------------------------

export function RenameDialog({ item, onClose }: Base & { item: Entry }) {
  const connId = useNavStore((s) => s.connectionId);
  const [name, setName] = React.useState(item.name);
  const folder = item.type === 'folder';
  const summary = useFolderSummary(connId, item.key, folder);
  const siblings = React.useMemo(
    () =>
      visibleEntries()
        .filter((e) => e.type === item.type && e.key !== item.key)
        .map((e) => e.name),
    [item],
  );
  const error = validateName(name, parentPrefix(item.key), siblings);
  const unchanged = name.trim() === item.name;
  const inputRef = React.useRef<HTMLInputElement>(null);
  React.useEffect(() => {
    // 拡張子を除いた部分を選択する
    const el = inputRef.current;
    if (!el) return;
    el.focus();
    const dot = folder ? -1 : item.name.lastIndexOf('.');
    el.setSelectionRange(0, dot > 0 ? dot : item.name.length);
  }, [folder, item.name]);
  const submit = () => {
    if (error || unchanged) return;
    onClose();
    void rename(item, name.trim());
  };
  return (
    <Dialog
      open
      onClose={onClose}
      icon={Pencil}
      title={d.rename.title}
      onSubmit={submit}
      footer={
        <>
          <Cancel onClick={onClose} />
          <Button type="submit" disabled={Boolean(error) || unchanged}>
            {d.rename.confirm}
          </Button>
        </>
      }
    >
      <Field label={d.newFolder.name} error={unchanged ? undefined : (error ?? undefined)}>
        {(p) => <Input {...p} ref={inputRef} value={name} onChange={(e) => setName(e.target.value)} />}
      </Field>
      {folder ? (
        <Note>{d.rename.folderNote(summary.data?.truncated ? null : (summary.data?.itemCount ?? null))}</Note>
      ) : null}
      {isVersioned(connId) ? <Note>{d.move.versioningNote}</Note> : null}
    </Dialog>
  );
}

// ---- DLG-02 削除の確認 -------------------------------------------------------------------

export function DeleteDialog({ items, onClose }: Base & { items: Entry[] }) {
  const connId = useNavStore((s) => s.connectionId);
  const deleted = items.every((e) => e.deleted);
  const versioned = isVersioned(connId) && !deleted;
  const [all, setAll] = React.useState(deleted);
  const folders = items.filter((e) => e.type === 'folder');
  const [firstFolder] = folders;
  const summary = useFolderSummary(connId, firstFolder?.key ?? '', folders.length === 1);
  const [first] = items;
  const title =
    items.length === 1 && first ? d.delete.titleOne(first.name) : d.delete.titleMany(items.length);
  const permanent = !versioned || all;
  const submit = () => {
    onClose();
    void deleteEntries(items, permanent && isVersioned(connId));
  };
  return (
    <Dialog
      open
      alert
      onClose={onClose}
      icon={Trash2}
      tone="destructive"
      title={title}
      description={permanent ? d.delete.permanent : d.delete.marker}
      footer={
        <>
          <Cancel onClick={onClose} />
          <Button variant="destructive" onClick={submit} autoFocus>
            {all && versioned ? d.delete.confirmAll : d.delete.confirm}
          </Button>
        </>
      }
    >
      {folders.length > 0 ? (
        <Note>
          {d.delete.folderContents(
            folders.length === 1 && summary.data && !summary.data.truncated ? summary.data.itemCount : null,
          )}
        </Note>
      ) : null}
      {versioned ? <Checkbox checked={all} onCheckedChange={setAll} label={d.delete.allVersions} /> : null}
    </Dialog>
  );
}

// ---- DLG-03 移動 -------------------------------------------------------------------------

interface TreeProps {
  connId: string;
  prefix: string;
  depth: number;
  dest: string | null;
  onPick: (prefix: string) => void;
  blocked: (prefix: string) => boolean;
}

function FolderNode({
  connId,
  folder,
  depth,
  dest,
  onPick,
  blocked,
}: Omit<TreeProps, 'prefix'> & { folder: { key: string; name: string } }) {
  const [open, setOpen] = React.useState(false);
  const disabled = blocked(folder.key);
  const selected = dest === folder.key;
  return (
    <>
      <div
        role="treeitem"
        aria-selected={selected}
        aria-expanded={open}
        aria-disabled={disabled || undefined}
        tabIndex={disabled ? -1 : 0}
        onClick={() => !disabled && onPick(folder.key)}
        onKeyDown={(e) => {
          if (e.key === 'ArrowRight') setOpen(true);
          if (e.key === 'ArrowLeft') setOpen(false);
          if ((e.key === ' ' || e.key === 'Enter') && !disabled) {
            e.preventDefault();
            onPick(folder.key);
          }
        }}
        className={cn(
          'flex h-7 items-center gap-1.5 rounded-md pr-2 text-base outline-none focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]',
          selected ? 'bg-row-selected text-row-selected-foreground' : 'hover:bg-row-hover',
          disabled && 'opacity-40',
        )}
        style={{ paddingLeft: 4 + depth * 18 }}
      >
        <button
          type="button"
          tabIndex={-1}
          aria-label={open ? '閉じる' : '開く'}
          onClick={(e) => {
            e.stopPropagation();
            setOpen((v) => !v);
          }}
          className="grid size-4 place-items-center rounded-xs outline-none"
        >
          <Icon icon={ChevronRight} size={12} className={cn('transition-transform', open && 'rotate-90')} />
        </button>
        <FileIcon folder />
        <span className="truncate">{folder.name}</span>
      </div>
      {open ? (
        <FolderChildren
          connId={connId}
          prefix={folder.key}
          depth={depth + 1}
          dest={dest}
          onPick={onPick}
          blocked={blocked}
        />
      ) : null}
    </>
  );
}

function FolderChildren(props: TreeProps) {
  const children = useFolderChildren(props.connId, props.prefix, true);
  if (children.isPending) {
    return (
      <div className="flex h-7 items-center" style={{ paddingLeft: 26 + props.depth * 18 }}>
        <Spinner size={12} />
      </div>
    );
  }
  return (
    <div role="group">
      {(children.data ?? [])
        .filter((e) => e.type === 'folder' && !e.deleted)
        .map((f) => (
          <FolderNode key={f.key} {...props} folder={f} />
        ))}
    </div>
  );
}

export function MoveDialog({ items, onClose }: Base & { items: Entry[] }) {
  const connId = useNavStore((s) => s.connectionId) ?? '';
  const [dest, setDest] = React.useState<string | null>(null);
  const [first] = items;
  const archived = items.filter(isArchived).length;
  const parents = new Set(items.map((e) => parentPrefix(e.key)));
  const blocked = (prefix: string) =>
    items.some((m) => m.type === 'folder' && prefix.startsWith(m.key)) ||
    (parents.size === 1 && parents.has(prefix));
  const submit = () => {
    if (dest === null) return;
    onClose();
    void move(items, dest);
  };
  return (
    <Dialog
      open
      onClose={onClose}
      icon={FolderInput}
      width={440}
      title={items.length === 1 && first ? d.move.titleOne(first.name) : d.move.titleMany(items.length)}
      description={d.move.description}
      footer={
        <>
          <Cancel onClick={onClose} />
          <Button onClick={submit} disabled={dest === null}>
            {ja.common.move}
          </Button>
        </>
      }
    >
      <div
        role="tree"
        aria-label={d.move.description}
        className="max-h-60 overflow-auto rounded-lg p-1 shadow-[inset_0_0_0_0.5px_var(--border)]"
      >
        <div
          role="treeitem"
          aria-selected={dest === ''}
          aria-disabled={blocked('') || undefined}
          tabIndex={0}
          onClick={() => !blocked('') && setDest('')}
          onKeyDown={(e) => {
            if ((e.key === ' ' || e.key === 'Enter') && !blocked('')) {
              e.preventDefault();
              setDest('');
            }
          }}
          className={cn(
            'flex h-7 items-center gap-2 rounded-md px-2 text-base outline-none focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]',
            dest === '' ? 'bg-row-selected text-row-selected-foreground' : 'hover:bg-row-hover',
            blocked('') && 'opacity-40',
          )}
        >
          <FileIcon folder />
          {d.move.root}
        </div>
        <FolderChildren connId={connId} prefix="" depth={1} dest={dest} onPick={setDest} blocked={blocked} />
      </div>
      {isVersioned(connId) ? <Note>{d.move.versioningNote}</Note> : null}
      {archived > 0 ? <Note>{d.move.archivedNote(archived)}</Note> : null}
    </Dialog>
  );
}

// ---- DLG-04 ストレージクラスを変更 ---------------------------------------------------------

export function StorageClassDialog({
  items,
  connection,
  onClose,
}: Base & { items: Entry[]; connection: Connection | null }) {
  const pricing = usePricing(connection?.region ?? null);
  const files = items.filter((e): e is Entry & { type: 'file' } => e.type === 'file');
  const [firstFile] = files;
  const current =
    firstFile &&
    files.length === items.length &&
    files.every((f) => f.storageClass === firstFile.storageClass)
      ? firstFile.storageClass
      : null;
  const [value, setValue] = React.useState<StorageClass>(
    current && current !== 'OTHER' ? current : 'STANDARD',
  );
  const archived = items.filter(isArchived).length;
  const [first] = items;
  const fromMinDuration = files.some((f) => (classInfo(f.storageClass)?.minDays ?? 0) > 0);
  const small = files.some((f) => f.size < 128 * 1024) && (classInfo(value)?.minSize128k ?? false);
  const submit = () => {
    if (value === current) return;
    onClose();
    void changeStorageClass(items, value);
  };
  return (
    <Dialog
      open
      onClose={onClose}
      icon={Layers}
      width={500}
      title={d.storageClass.title}
      description={
        items.length === 1 && first
          ? d.storageClass.descOne(first.name)
          : d.storageClass.descMany(items.length)
      }
      footer={
        <>
          <Cancel onClick={onClose} />
          <Button onClick={submit} disabled={value === current}>
            {ja.common.apply}
          </Button>
        </>
      }
    >
      <div role="radiogroup" aria-label={d.storageClass.title} className="flex flex-col gap-1">
        {SELECTABLE_CLASSES.map((k) => {
          const info = STORAGE_CLASSES[k];
          const selected = value === k;
          const price = pricing.data?.perGbMonth[k];
          return (
            <div
              key={k}
              role="radio"
              aria-checked={selected}
              tabIndex={selected ? 0 : -1}
              onClick={() => setValue(k)}
              onKeyDown={(e) => {
                const i = SELECTABLE_CLASSES.indexOf(k);
                const next =
                  e.key === 'ArrowDown' || e.key === 'ArrowRight'
                    ? SELECTABLE_CLASSES[(i + 1) % SELECTABLE_CLASSES.length]
                    : e.key === 'ArrowUp' || e.key === 'ArrowLeft'
                      ? SELECTABLE_CLASSES[(i - 1 + SELECTABLE_CLASSES.length) % SELECTABLE_CLASSES.length]
                      : undefined;
                if (next) {
                  e.preventDefault();
                  setValue(next);
                  const parent = e.currentTarget.parentElement;
                  queueMicrotask(() => parent?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus());
                }
              }}
              className={cn(
                'flex items-center gap-2.5 rounded-lg px-2.5 py-1.75 outline-none focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]',
                selected
                  ? 'bg-[color-mix(in_oklch,var(--primary)_6%,transparent)] shadow-[inset_0_0_0_1.5px_var(--primary)]'
                  : 'shadow-[inset_0_0_0_0.5px_var(--border)]',
              )}
            >
              <span
                className={cn(
                  'size-3.5 shrink-0 rounded-full bg-field',
                  selected
                    ? 'shadow-[inset_0_0_0_4px_var(--primary)]'
                    : 'shadow-[inset_0_0_0_1px_var(--input)]',
                )}
              />
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-1.5">
                  <StorageClassBadge value={k} short={false} plain className="text-base font-medium" />
                  {k === current ? <Badge variant="outline">{d.storageClass.current}</Badge> : null}
                  {info.slow ? (
                    <Badge variant="warning" icon={Clock}>
                      {d.storageClass.retrieval(info.retrieval)}
                    </Badge>
                  ) : null}
                </div>
                <div className="mt-0.5 pl-3.25 text-xs text-muted-foreground">
                  {info.description}
                  {info.minDays ? ` · ${d.storageClass.minDays(info.minDays)}` : ''}
                </div>
              </div>
              <div className="text-sm whitespace-nowrap text-muted-foreground tabular-nums">
                {price != null ? d.storageClass.perGb(formatUsd(price)) : ''}
              </div>
            </div>
          );
        })}
      </div>
      {isVersioned(connection?.id ?? null) ? <Note>{d.storageClass.versioningNote}</Note> : null}
      {fromMinDuration ? <Note>{d.storageClass.minDurationNote}</Note> : null}
      {small ? <Note>{d.storageClass.smallObjectNote}</Note> : null}
      {archived > 0 ? <Note>{d.storageClass.archivedNote(archived)}</Note> : null}
      {items.length >= 1000 ? <Note>{d.storageClass.lifecycleNote}</Note> : null}
    </Dialog>
  );
}

// ---- DLG-07 アーカイブからの取り出し --------------------------------------------------------

export function RestoreDialog({ items, onClose }: Base & { items: Entry[] }) {
  const files = items.filter((e): e is Entry & { type: 'file' } => e.type === 'file');
  const [first] = files;
  const deep = files.some((f) => f.storageClass === 'DEEP_ARCHIVE');
  const tiering = files.length > 0 && files.every((f) => f.storageClass === 'INTELLIGENT_TIERING');
  const [tier, setTier] = React.useState<RestoreTier>('standard');
  const [days, setDays] = React.useState(7);
  const r = d.restore;
  const submit = () => {
    onClose();
    void requestRestore(files, tier, tiering ? undefined : days);
  };
  const tiers: { value: RestoreTier; label: string }[] = [
    ...(deep ? [] : [{ value: 'expedited' as const, label: r.expedited }]),
    { value: 'standard', label: r.standard },
    { value: 'bulk', label: r.bulk },
  ];
  return (
    <Dialog
      open
      onClose={onClose}
      icon={ArchiveRestore}
      width={460}
      title={files.length === 1 && first ? r.titleOne(first.name) : r.titleMany(files.length)}
      description={r.description(first ? classLabel(first.storageClass) : '')}
      onSubmit={submit}
      footer={
        <>
          <Cancel onClick={onClose} />
          <Button type="submit" disabled={files.length === 0}>
            {r.confirm}
          </Button>
        </>
      }
    >
      <Field label={r.tier}>
        {(p) => (
          <NativeSelect
            id={p.id}
            value={tier}
            onChange={(e) => setTier(e.target.value as RestoreTier)}
            options={tiers}
          />
        )}
      </Field>
      {tiering ? null : (
        <Field label={r.days}>
          {(p) => (
            <NativeSelect
              id={p.id}
              wrapperClassName="w-32"
              value={String(days)}
              onChange={(e) => setDays(Number(e.target.value))}
              options={Array.from({ length: 30 }, (_, i) => ({
                value: String(i + 1),
                label: r.daysUnit(i + 1),
              }))}
            />
          )}
        </Field>
      )}
      <Note>{r.note}</Note>
    </Dialog>
  );
}

// ---- DLG-08 同名の項目がある場合の確認 -------------------------------------------------------

export function ConflictDialog({
  conflicts,
  versioned,
  resolve,
}: {
  conflicts: RemoteConflict[] | UploadConflict[];
  versioned: boolean;
  resolve: ConflictResolver;
}) {
  const [index, setIndex] = React.useState(0);
  const [applyAll, setApplyAll] = React.useState(false);
  const decisions = React.useRef<Record<string, ConflictDecision>>({});
  const current = conflicts[index];
  const remaining = conflicts.length - index - 1;
  const c = d.conflict;
  const decide = (decision: ConflictDecision) => {
    const list = applyAll ? conflicts.slice(index) : current ? [current] : [];
    for (const item of list) decisions.current[item.key] = decision;
    const next = applyAll ? conflicts.length : index + 1;
    if (next >= conflicts.length) resolve({ ...decisions.current });
    else setIndex(next);
  };
  if (!current) return null;
  return (
    <Dialog
      open
      alert
      onClose={() => resolve(null)}
      icon={CopyPlus}
      width={480}
      title={c.title(baseName(current.key))}
      description={
        <>
          {versioned ? c.versioned : c.unversioned}
          <span className="mt-1 block text-sm tabular-nums">
            {formatSize(current.remoteSize)} · {formatDate(current.remoteModified)}
          </span>
        </>
      }
      footer={
        <>
          <Button variant="outline" onClick={() => resolve(null)}>
            {c.abort}
          </Button>
          <span className="flex-1" />
          <Button variant="outline" onClick={() => decide('skip')}>
            {c.skip}
          </Button>
          <Button variant="outline" onClick={() => decide('keepBoth')}>
            {c.keepBoth}
          </Button>
          <Button onClick={() => decide('replace')} autoFocus>
            {c.replace}
          </Button>
        </>
      }
    >
      {remaining > 0 ? (
        <Checkbox checked={applyAll} onCheckedChange={setApplyAll} label={c.applyAll(remaining)} />
      ) : null}
    </Dialog>
  );
}

// ---- DLG-09 バージョンの完全削除の確認 ------------------------------------------------------

export function DeleteVersionDialog({
  versionKey,
  versionId,
  date,
  size,
  isLatest,
  onClose,
}: Base & { versionKey: string; versionId: string; date: string; size: number | null; isLatest: boolean }) {
  const v = d.deleteVersion;
  const submit = () => {
    onClose();
    void deleteVersion(versionKey, versionId);
  };
  return (
    <Dialog
      open
      alert
      onClose={onClose}
      icon={Trash2}
      tone="destructive"
      title={v.title}
      description={
        <>
          {v.description(formatDate(date), formatSize(size))}
          {isLatest ? <> {v.latestNote}</> : null}
        </>
      }
      footer={
        <>
          <Cancel onClick={onClose} />
          <Button variant="destructive" onClick={submit} autoFocus>
            {v.confirm}
          </Button>
        </>
      }
    />
  );
}
