// SCR-04 設定（別ウィンドウ、560×480）。変更は即時に反映して保存する（03 §7）。

import { useQueries, useQueryClient } from '@tanstack/react-query';
import { Database, FolderOpen, RefreshCw, Trash2, TriangleAlert } from 'lucide-react';
import * as React from 'react';
import { applyAppearance, useSharedEvents } from '@/app/app-events';
import { useBucketTags, useConnections, useSettings } from '@/app/queries';
import { qk } from '@/app/query-keys';
import { Icon } from '@/components/ds/icon';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Skeleton, Spinner } from '@/components/ui/misc';
import { NativeSelect } from '@/components/ui/native-select';
import { SegmentedControl } from '@/components/ui/segmented-control';
import { SwitchRow } from '@/components/ui/switch';
import { toast } from '@/components/ui/toaster';
import { checkForUpdate } from '@/features/commands';
import { DialogHost } from '@/features/dialogs/dialog-host';
import { showError } from '@/features/errors';
import { formatDate } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import type {
  Appearance,
  Connection,
  ConnectionPatch,
  CostTag,
  SettingsPatch,
  StorageClass,
  UserSession,
} from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { classLabel, SELECTABLE_CLASSES, STORAGE_CLASSES } from '@/lib/storage-class';
import { useUiStore } from '@/stores/ui';

const t = ja.settings;

type Tab = keyof typeof t.tabs;

function Row({
  label,
  children,
  hint,
}: {
  label: string;
  children: React.ReactNode;
  hint?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 text-base">
      <div className="flex min-w-0 flex-col gap-0.5">
        <span>{label}</span>
        {hint ? <span className="text-xs text-muted-foreground">{hint}</span> : null}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

function Section({ title, children }: { title?: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      {title ? <h2 className="m-0 text-sm font-semibold text-muted-foreground">{title}</h2> : null}
      {children}
    </section>
  );
}

function useUpdateSettings() {
  const queryClient = useQueryClient();
  return React.useCallback(
    (patch: SettingsPatch) =>
      ipc.app
        .updateSettings(patch)
        .then((s) => queryClient.setQueryData(qk.settings, s))
        .catch((e) => showError(e, '設定を変更')),
    [queryClient],
  );
}

/** 接続ごとの設定の変更（確認は不要。05 §3.2）。 */
function usePatchConnection() {
  const queryClient = useQueryClient();
  return React.useCallback(
    (id: string, patch: ConnectionPatch, verb: string) =>
      ipc.connections
        .patch(id, patch)
        .then(() => queryClient.invalidateQueries({ queryKey: qk.connections }))
        .catch((e) => showError(e, verb)),
    [queryClient],
  );
}

const range = (from: number, to: number) =>
  Array.from({ length: to - from + 1 }, (_, i) => ({ value: String(from + i), label: String(from + i) }));

function GeneralTab() {
  const settings = useSettings().data;
  const update = useUpdateSettings();
  const queryClient = useQueryClient();
  if (!settings) return <Skeleton className="h-40" />;
  const g = settings.general;
  const setTheme = (v: Appearance) => {
    applyAppearance(v);
    ipc.app
      .setTheme(v)
      .then(() => ipc.app.settings())
      .then((s) => queryClient.setQueryData(qk.settings, s))
      .catch((e) => showError(e, '外観を変更'));
  };
  return (
    <Section>
      <Row label={t.appearance}>
        <SegmentedControl<Appearance>
          aria-label={t.appearance}
          value={g.appearance}
          onChange={setTheme}
          options={[
            { value: 'auto', label: t.appearanceAuto },
            { value: 'light', label: t.appearanceLight },
            { value: 'dark', label: t.appearanceDark },
          ]}
        />
      </Row>
      <Row label={t.downloadDir}>
        <span
          className="max-w-60 truncate text-sm text-muted-foreground"
          title={g.downloadDir ?? '~/Downloads'}
        >
          {g.downloadDir ?? '~/Downloads'}
        </span>
        <Button
          variant="outline"
          size="sm"
          onClick={() =>
            ipc.app
              .chooseDownloadDir()
              .then((s) => queryClient.setQueryData(qk.settings, s))
              .catch((e) => showError(e, 'ダウンロード先を変更'))
          }
        >
          {ja.common.change}
        </Button>
      </Row>
      <SwitchRow
        label={t.showHidden}
        checked={g.showHidden}
        onCheckedChange={(v) => void update({ general: { showHidden: v } })}
      />
      <SwitchRow
        label={t.showMenuBarIcon}
        checked={g.showMenuBarIcon}
        onCheckedChange={(v) => void update({ general: { showMenuBarIcon: v } })}
      />
    </Section>
  );
}

/** 接続ごとのアップロード時のストレージクラス（未設定なら全体の設定に従う。04 §4.3）。 */
function ConnectionClassRow({
  connection,
  fallback,
}: Readonly<{ connection: Connection; fallback: StorageClass }>) {
  const patch = usePatchConnection();
  return (
    <Row label={connection.bucket}>
      <NativeSelect
        size="sm"
        aria-label={`${connection.bucket} ${t.defaultStorageClass}`}
        wrapperClassName="w-52"
        value={connection.defaultStorageClass ?? ''}
        onChange={(e) =>
          void patch(
            connection.id,
            { defaultStorageClass: (e.target.value || null) as StorageClass | null },
            'ストレージクラスを変更',
          )
        }
        options={[
          { value: '', label: t.followDefault(classLabel(fallback)) },
          ...SELECTABLE_CLASSES.map((c) => ({ value: c, label: STORAGE_CLASSES[c].label })),
        ]}
      />
    </Row>
  );
}

function TransferTab({ connections }: Readonly<{ connections: Connection[] }>) {
  const settings = useSettings().data;
  const update = useUpdateSettings();
  if (!settings) return <Skeleton className="h-40" />;
  const x = settings.transfer;
  const ignoresDsStore = x.ignore.includes('.DS_Store');
  return (
    <>
      <Section>
        <Row label={t.maxFiles}>
          <NativeSelect
            size="sm"
            aria-label={t.maxFiles}
            wrapperClassName="w-20"
            value={String(x.maxFiles)}
            onChange={(e) => void update({ transfer: { maxFiles: Number(e.target.value) } })}
            options={range(1, 8)}
          />
        </Row>
        <Row label={t.maxParts}>
          <NativeSelect
            size="sm"
            aria-label={t.maxParts}
            wrapperClassName="w-20"
            value={String(x.maxPartsPerFile)}
            onChange={(e) => void update({ transfer: { maxPartsPerFile: Number(e.target.value) } })}
            options={range(1, 16)}
          />
        </Row>
        <Row label={t.multipartThreshold}>
          <NativeSelect
            size="sm"
            aria-label={t.multipartThreshold}
            wrapperClassName="w-24"
            value={String(x.multipartThresholdMb)}
            onChange={(e) => void update({ transfer: { multipartThresholdMb: Number(e.target.value) } })}
            options={[8, 16, 32, 64].map((v) => ({ value: String(v), label: `${v} MB` }))}
          />
        </Row>
        <Row label={t.defaultStorageClass}>
          <NativeSelect
            size="sm"
            aria-label={t.defaultStorageClass}
            wrapperClassName="w-52"
            value={x.defaultStorageClass}
            onChange={(e) =>
              void update({ transfer: { defaultStorageClass: e.target.value as StorageClass } })
            }
            options={SELECTABLE_CLASSES.map((c) => ({ value: c, label: STORAGE_CLASSES[c].label }))}
          />
        </Row>
        <SwitchRow
          label={t.normalizeNfc}
          checked={x.normalizeNfc}
          onCheckedChange={(v) => void update({ transfer: { normalizeNfc: v } })}
        />
        <SwitchRow
          label={t.ignoreDsStore}
          checked={ignoresDsStore}
          onCheckedChange={(v) =>
            void update({
              transfer: {
                ignore: v
                  ? [...x.ignore.filter((i) => i !== '.DS_Store'), '.DS_Store']
                  : x.ignore.filter((i) => i !== '.DS_Store'),
              },
            })
          }
        />
        <SwitchRow
          label={t.notifyOnComplete}
          checked={x.notifyOnComplete}
          onCheckedChange={(v) => void update({ transfer: { notifyOnComplete: v } })}
        />
      </Section>
      {connections.length > 0 ? (
        <Section title={t.connectionStorageClass}>
          <p className="m-0 text-xs text-muted-foreground">{t.connectionStorageClassNote}</p>
          {connections.map((c) => (
            <ConnectionClassRow key={c.id} connection={c} fallback={x.defaultStorageClass} />
          ))}
        </Section>
      ) : null}
    </>
  );
}

function ConnectionsTab({ connections }: { connections: Connection[] }) {
  const open = useUiStore((s) => s.openDialog);
  const patch = usePatchConnection();
  const roles = connections.filter((c) => c.roleArn);
  return (
    <>
      <Section>
        <ul className="m-0 flex list-none flex-col rounded-lg p-0 shadow-[inset_0_0_0_0.5px_var(--border)]">
          {connections.map((c) => (
            <li key={c.id} className="flex items-center gap-2.5 px-3 py-2 not-last:hairline-b">
              <Icon icon={Database} size={16} className="text-primary" />
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-base font-medium">{c.bucket}</span>
                <span className="truncate text-xs text-muted-foreground">
                  {c.regionShort} · {c.roleArn ? t.authRole : t.authStatic}（{c.accessKeyIdMasked}）
                </span>
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={() => open({ type: 'editConnection', connectionId: c.id })}
              >
                {t.edit}
              </Button>
              <Button
                variant="outline"
                size="sm"
                className="text-destructive"
                onClick={() => open({ type: 'deleteConnection', connectionId: c.id })}
              >
                {t.remove}
              </Button>
            </li>
          ))}
        </ul>
        <Button variant="outline" className="self-start" onClick={() => open({ type: 'addBucket' })}>
          {t.addBucket}
        </Button>
      </Section>
      {roles.length > 0 ? (
        // AssumeRole の SourceIdentity（接続ごと、既定はオフ。04 §2.3）
        <Section title={t.sourceIdentity}>
          <p className="m-0 text-xs text-muted-foreground">{t.sourceIdentityNote}</p>
          {roles.map((c) => (
            <SwitchRow
              key={c.id}
              label={c.bucket}
              checked={c.useSourceIdentity}
              onCheckedChange={(v) =>
                void patch(c.id, { useSourceIdentity: v }, 'SourceIdentity の設定を変更')
              }
            />
          ))}
        </Section>
      ) : null}
    </>
  );
}

const sameTag = (a: CostTag | null, b: CostTag | null) =>
  (a?.key ?? '') === (b?.key ?? '') && (a?.value ?? '') === (b?.value ?? '');

/** コスト配分タグのキーと値の入力欄。バケットのタグを読み取れないときと、「手入力…」を選んだときに使う。 */
function CostTagInputs({
  connection,
  onSave,
}: Readonly<{ connection: Connection; onSave: (next: CostTag | null) => void }>) {
  const [key, setKey] = React.useState(connection.costTag?.key ?? '');
  const [value, setValue] = React.useState(connection.costTag?.value ?? '');
  const save = () => {
    const k = key.trim();
    const v = value.trim();
    onSave(k && v ? { key: k, value: v } : null);
  };
  return (
    <div className="grid grid-cols-2 gap-2">
      <Input
        size="sm"
        aria-label={`${connection.bucket} ${t.costTagKey}`}
        placeholder={t.costTagKey}
        value={key}
        onChange={(e) => setKey(e.target.value)}
        onBlur={save}
      />
      <Input
        size="sm"
        aria-label={`${connection.bucket} ${t.costTagValue}`}
        placeholder={t.costTagValue}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onBlur={save}
      />
    </div>
  );
}

/**
 * 接続のコスト配分タグ（04 §13.6）。バケットに付いているタグを候補として示し、そこから選ぶ。
 * 読み取ったタグを自動では使わない（どのキーが Billing で有効化されているかは判別できないため）。
 */
function CostTagRow({ connection }: Readonly<{ connection: Connection }>) {
  const patch = usePatchConnection();
  const tags = useBucketTags(connection.id);
  const [manual, setManual] = React.useState(false);
  const cur = connection.costTag;
  const save = (next: CostTag | null) => {
    if (!sameTag(next, cur)) void patch(connection.id, { costTag: next }, 'コスト配分タグを変更');
  };
  // 値がバケット名のタグ（バケット単位のコストになる）を先頭にする
  const candidates = [...(tags.data ?? [])].sort(
    (a, b) => Number(b.value === connection.bucket) - Number(a.value === connection.bucket),
  );
  const index = candidates.findIndex((c) => sameTag(c, cur));
  const missing = tags.isSuccess && cur != null && index < 0;
  const selected = manual ? 'manual' : cur == null ? 'none' : index >= 0 ? `tag:${index}` : 'current';
  const choose = (v: string) => {
    setManual(v === 'manual');
    if (v === 'none') save(null);
    else if (v.startsWith('tag:')) save(candidates[Number(v.slice(4))] ?? null);
  };
  return (
    <div className="flex flex-col gap-1.5">
      <div className="grid grid-cols-[1fr_2fr] items-center gap-2">
        <span className="truncate text-base">{connection.bucket}</span>
        {tags.isPending ? (
          <Skeleton className="h-6" />
        ) : tags.isError ? (
          <CostTagInputs connection={connection} onSave={save} />
        ) : (
          <NativeSelect
            size="sm"
            aria-label={`${connection.bucket} ${t.costTag}`}
            value={selected}
            onChange={(e) => choose(e.target.value)}
            options={[
              { value: 'none', label: t.costTagNone },
              ...candidates.map((c, i) => ({ value: `tag:${i}`, label: `${c.key}=${c.value}` })),
              ...(missing
                ? [{ value: 'current', label: t.costTagNotOnBucket(`${cur.key}=${cur.value}`) }]
                : []),
              { value: 'manual', label: t.costTagManual },
            ]}
          />
        )}
      </div>
      {tags.isSuccess && manual ? (
        <div className="grid grid-cols-[1fr_2fr] gap-2">
          <span />
          <CostTagInputs connection={connection} onSave={save} />
        </div>
      ) : null}
      {missing ? (
        <p role="alert" className="m-0 flex items-start gap-1 text-xs">
          <Icon icon={TriangleAlert} size={12} className="mt-0.5 shrink-0 text-warning" />
          <span className="min-w-0">{t.costTagMissing(cur.key, cur.value)}</span>
        </p>
      ) : null}
      {tags.isSuccess && candidates.length === 0 && !cur ? (
        <p className="m-0 text-xs text-muted-foreground">{t.costTagNoTags}</p>
      ) : null}
      {tags.isError ? (
        <p className="m-0 text-xs text-muted-foreground">
          {t.costTagUnreadable(ipc.toAppError(tags.error).message)}
        </p>
      ) : null}
    </div>
  );
}

function CostTab({ connections }: { connections: Connection[] }) {
  const settings = useSettings().data;
  const update = useUpdateSettings();
  if (!settings) return <Skeleton className="h-40" />;
  return (
    <>
      <Section>
        <SwitchRow
          label={t.useCostExplorer}
          description={t.costExplorerNote}
          checked={settings.cost.useCostExplorer}
          onCheckedChange={(v) => void update({ cost: { useCostExplorer: v } })}
        />
      </Section>
      <Section title={t.costTag}>
        <p className="m-0 text-xs text-muted-foreground">{t.costTagNote}</p>
        {connections.map((c) => (
          <CostTagRow key={c.id} connection={c} />
        ))}
      </Section>
    </>
  );
}

function IndexRows({ connections }: { connections: Connection[] }) {
  const queryClient = useQueryClient();
  const [busy, setBusy] = React.useState<Record<string, number | null>>({});
  const statuses = useQueries({
    queries: connections.map((c) => ({
      queryKey: qk.indexStatus(c.id),
      queryFn: () => ipc.search.status(c.id),
    })),
  });
  const rebuild = (id: string) => {
    setBusy((b) => ({ ...b, [id]: 0 }));
    ipc.search
      .rebuild(id, (e) => {
        if (e.event === 'progress') setBusy((b) => ({ ...b, [id]: e.data.scanned }));
        if (e.event === 'finished' || e.event === 'failed') {
          setBusy(({ [id]: _, ...rest }) => rest);
          void queryClient.invalidateQueries({ queryKey: qk.indexStatus(id) });
          if (e.event === 'failed') showError(e.data.error, '検索インデックスを再構築');
        }
      })
      .catch((e) => {
        setBusy(({ [id]: _, ...rest }) => rest);
        showError(e, '検索インデックスを再構築');
      });
  };
  const remove = (id: string) =>
    ipc.search
      .remove(id)
      .then(() => queryClient.invalidateQueries({ queryKey: qk.indexStatus(id) }))
      .catch((e) => showError(e, '検索インデックスを削除'));
  return (
    <div className="flex flex-col gap-2">
      {connections.map((c, i) => {
        const s = statuses[i]?.data;
        const running = c.id in busy;
        return (
          <div key={c.id} className="flex items-center gap-2 text-base">
            <span className="min-w-0 flex-1 truncate">{c.bucket}</span>
            <span className="text-sm text-muted-foreground tabular-nums">
              {running
                ? ja.filter.indexBuilding(busy[c.id] ?? 0)
                : s && s.state !== 'none'
                  ? t.indexSummary(s.objectCount, formatDate(s.lastScanAt))
                  : t.indexNone}
            </span>
            <Button variant="outline" size="sm" disabled={running} onClick={() => rebuild(c.id)}>
              {running ? <Spinner size={12} /> : <Icon icon={RefreshCw} size={12} />}
              {t.rebuild}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="text-destructive"
              disabled={running}
              onClick={() => void remove(c.id)}
            >
              <Icon icon={Trash2} size={12} />
              {t.removeIndex}
            </Button>
          </div>
        );
      })}
    </div>
  );
}

function AdvancedTab({ connections }: { connections: Connection[] }) {
  const settings = useSettings().data;
  const update = useUpdateSettings();
  if (!settings) return <Skeleton className="h-40" />;
  return (
    <>
      {connections.length > 0 ? (
        <Section title={t.searchIndex}>
          <IndexRows connections={connections} />
        </Section>
      ) : null}
      <Section>
        <Row label={t.autoRefresh}>
          <NativeSelect
            size="sm"
            aria-label={t.autoRefresh}
            wrapperClassName="w-72"
            value={String(settings.search.autoRefreshMinutes > 0 ? 60 : 0)}
            onChange={(e) => void update({ search: { autoRefreshMinutes: Number(e.target.value) } })}
            options={[
              { value: '60', label: t.autoRefresh60 },
              { value: '0', label: t.autoRefreshManual },
            ]}
          />
        </Row>
        <Row label={t.logLevel}>
          <NativeSelect
            size="sm"
            aria-label={t.logLevel}
            wrapperClassName="w-28"
            value={settings.advanced.logLevel}
            onChange={(e) => void update({ advanced: { logLevel: e.target.value as 'info' | 'debug' } })}
            options={[
              { value: 'info', label: t.logInfo },
              { value: 'debug', label: t.logDebug },
            ]}
          />
        </Row>
        <Row label={t.logs}>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void ipc.app.openLogs().catch((e) => showError(e, t.openLogs))}
          >
            <Icon icon={FolderOpen} size={12} />
            {t.openLogs}
          </Button>
        </Row>
        <Row label={t.cache}>
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              ipc.app
                .clearCache()
                .then(() => toast.show({ tone: 'success', title: t.cacheCleared }))
                .catch((e) => showError(e, t.clearCache))
            }
          >
            {t.clearCache}
          </Button>
        </Row>
        <SwitchRow
          label={t.autoUpdate}
          checked={settings.advanced.autoCheckUpdate}
          onCheckedChange={(v) => void update({ advanced: { autoCheckUpdate: v } })}
        />
        <Button variant="outline" size="sm" className="self-end" onClick={() => void checkForUpdate()}>
          {t.checkNow}
        </Button>
      </Section>
    </>
  );
}

export function SettingsWindow() {
  const [tab, setTab] = React.useState<Tab>('general');
  const [session, setSession] = React.useState<UserSession | null | undefined>(undefined);
  const connections = useConnections(Boolean(session));
  const settings = useSettings();
  useSharedEvents();

  React.useEffect(() => {
    ipc.app
      .startupInfo()
      .then((i) => setSession(i.session))
      .catch(() => setSession(null));
    const un = ipc.events.onSession(setSession);
    return () => void un.then((f) => f()).catch(() => {});
  }, []);

  React.useEffect(() => {
    if (settings.data) applyAppearance(settings.data.general.appearance);
  }, [settings.data]);

  const list = connections.data ?? [];
  const needsSession = tab === 'connections' || tab === 'cost';
  return (
    <div className="flex h-full flex-col bg-background">
      <header data-tauri-drag-region className="flex shrink-0 justify-center px-5 pt-3 pb-3 hairline-b">
        <SegmentedControl<Tab>
          aria-label={t.title}
          value={tab}
          onChange={setTab}
          options={(Object.keys(t.tabs) as Tab[]).map((k) => ({ value: k, label: t.tabs[k] }))}
        />
      </header>
      <main className="flex min-h-0 flex-1 flex-col gap-5 overflow-auto p-5">
        {tab === 'general' ? <GeneralTab /> : null}
        {tab === 'transfer' ? <TransferTab connections={session ? list : []} /> : null}
        {needsSession && session === null ? (
          <p className="m-0 text-base text-muted-foreground">{t.signedOut}</p>
        ) : null}
        {tab === 'connections' && session ? <ConnectionsTab connections={list} /> : null}
        {tab === 'cost' && session ? <CostTab connections={list} /> : null}
        {tab === 'advanced' ? <AdvancedTab connections={session ? list : []} /> : null}
      </main>
      {session ? <DialogHost connection={null} /> : null}
    </div>
  );
}
