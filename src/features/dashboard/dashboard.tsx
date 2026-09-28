// SCR-03 ストレージとコスト（03 §6）。DS: ui_kits/s3-drive/Dashboard.jsx。
// Cost Explorer への問い合わせは「更新」「取得」を押したときだけ行う（04 §13.4）。

import { useQueryClient } from '@tanstack/react-query';
import { Globe, HardDrive, History, type LucideIcon, Receipt, RefreshCw } from 'lucide-react';
import * as React from 'react';
import { create } from 'zustand';
import { useBucketInfo, useCost, usePricing, useSettings, useStorageMetrics } from '@/app/queries';
import { qk } from '@/app/query-keys';
import { DragSpacer } from '@/components/ds/app-shell';
import { Icon } from '@/components/ds/icon';
import { StorageClassBadge } from '@/components/ds/storage-class-badge';
import { UsageBar } from '@/components/ds/usage-bar';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { IconButton } from '@/components/ui/icon-button';
import { Skeleton, Spinner } from '@/components/ui/misc';
import { NativeSelect } from '@/components/ui/native-select';
import { Tooltip } from '@/components/ui/tooltip';
import { showError } from '@/features/errors';
import { formatDate, formatDelta, formatPercent, formatSize, formatUsd } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import type { AppError, Connection, CostSummary, StorageClass } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { cn } from '@/lib/utils';
import { useNavStore } from '@/stores/nav';

const t = ja.dashboard;

const CLASS_ORDER: StorageClass[] = [
  'STANDARD',
  'INTELLIGENT_TIERING',
  'STANDARD_IA',
  'ONEZONE_IA',
  'GLACIER_IR',
  'GLACIER',
  'DEEP_ARCHIVE',
  'OTHER',
];

const GB = 1e9;

interface DashState {
  busy: boolean;
  error: AppError | null;
}
const useDashStore = create<DashState>(() => ({ busy: false, error: null }));

function currentMonth(now = new Date()): string {
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
}

function monthLabel(month: string): string {
  const m = Number(month.split('-')[1]);
  return Number.isFinite(m) ? `${m}月` : month;
}

/** 「更新」: 利用容量と単価を取り直し、Cost Explorer に問い合わせる。 */
export function useDashboardRefresh(connection: Connection | null) {
  const queryClient = useQueryClient();
  const settings = useSettings();
  const useCe = settings.data?.cost.useCostExplorer ?? true;
  return React.useCallback(async () => {
    if (!connection) return;
    const id = connection.id;
    useDashStore.setState({ busy: true, error: null });
    const storage = ipc.metrics
      .storage(id, true)
      .then((m) => queryClient.setQueryData(qk.storageMetrics(id), m))
      .catch(() => {});
    void queryClient.invalidateQueries({ queryKey: qk.bucket(id) });
    void queryClient.invalidateQueries({ queryKey: qk.pricing(connection.region) });
    try {
      if (useCe) {
        const cost = await ipc.metrics.refreshCost(id);
        queryClient.setQueryData(qk.cost(id), cost);
      }
    } catch (e) {
      useDashStore.setState({ error: ipc.toAppError(e) });
    } finally {
      await storage;
      useDashStore.setState({ busy: false });
    }
  }, [connection, queryClient, useCe]);
}

export function DashboardToolbar({
  connections,
  connection,
}: {
  connections: Connection[];
  connection: Connection | null;
}) {
  const refresh = useDashboardRefresh(connection);
  const busy = useDashStore((s) => s.busy);
  return (
    <>
      <h1 className="m-0 text-base font-semibold">{t.title}</h1>
      <DragSpacer />
      <NativeSelect
        size="sm"
        aria-label="接続"
        wrapperClassName="w-[180px]"
        value={connection?.id ?? ''}
        onChange={(e) => {
          useDashStore.setState({ error: null });
          useNavStore.getState().openConnection(e.target.value);
        }}
        options={connections.map((c) => ({ value: c.id, label: c.bucket }))}
      />
      <IconButton
        icon={RefreshCw}
        label={ja.toolbar.refreshCost}
        disabled={busy || !connection}
        onClick={() => void refresh()}
      />
    </>
  );
}

function Card({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <section className={cn('min-w-0 rounded-xl bg-card p-4 elevation-sm', className)}>{children}</section>
  );
}

function CardHead({ children, right }: { children: React.ReactNode; right?: React.ReactNode }) {
  return (
    <div className="mb-3 flex items-baseline justify-between gap-2">
      <h2 className="m-0 text-base font-semibold">{children}</h2>
      {right}
    </div>
  );
}

function Kpi({
  icon,
  label,
  value,
  sub,
  loading,
}: {
  icon: LucideIcon;
  label: string;
  value: React.ReactNode;
  sub: React.ReactNode;
  loading?: boolean;
}) {
  return (
    <Card className="flex flex-col gap-1.5">
      <div className="flex items-center gap-1.5 text-xs font-semibold text-muted-foreground">
        <Icon icon={icon} size={13} />
        {label}
      </div>
      {loading ? (
        <>
          <Skeleton className="h-7 w-24" />
          <Skeleton className="h-3.5 w-32" />
        </>
      ) : (
        <>
          <div className="text-xl font-bold tracking-[-0.01em] tabular-nums">{value}</div>
          <div className="text-sm text-muted-foreground">{sub}</div>
        </>
      )}
    </Card>
  );
}

/** コスト系のカードの中身（未取得・無効・エラー）。 */
function CostPlaceholder({ connection }: { connection: Connection }) {
  const settings = useSettings();
  const refresh = useDashboardRefresh(connection);
  const { busy, error } = useDashStore();
  if (settings.data && !settings.data.cost.useCostExplorer) {
    return <p className="m-0 text-sm text-muted-foreground">{t.disabled}</p>;
  }
  return (
    <div className="flex flex-col items-start gap-2 text-sm text-muted-foreground">
      <span>{error ? error.message : t.notFetched}</span>
      <div className="flex items-center gap-2">
        <Button size="sm" variant="outline" disabled={busy} onClick={() => void refresh()}>
          {busy ? <Spinner size={12} /> : null}
          {t.fetch}
        </Button>
        <span className="text-xs">{t.fetchCost}</span>
      </div>
    </div>
  );
}

/**
 * CloudWatch のメトリクスも検索インデックスもないとき（04 §12.2）。インデックスを作れば、そこから集計する。
 */
function UsageFallback({ connection }: { connection: Connection }) {
  const queryClient = useQueryClient();
  const [scanned, setScanned] = React.useState<number | null>(null);
  const create = () => {
    setScanned(0);
    ipc.search
      .rebuild(connection.id, (e) => {
        if (e.event === 'progress') setScanned(e.data.scanned);
        if (e.event === 'finished') {
          setScanned(null);
          void queryClient.invalidateQueries({ queryKey: qk.storageMetrics(connection.id) });
          void queryClient.invalidateQueries({ queryKey: qk.indexStatus(connection.id) });
        }
        if (e.event === 'failed') {
          setScanned(null);
          showError(e.data.error, '検索インデックスを作成');
        }
      })
      .catch((e) => {
        setScanned(null);
        showError(e, '検索インデックスを作成');
      });
  };
  return (
    <div className="flex flex-col items-start gap-2 text-sm text-muted-foreground">
      <p className="m-0">{t.noMetricsYet}</p>
      <p className="m-0">{t.indexHint}</p>
      <div className="flex items-center gap-2">
        <Button size="sm" variant="outline" disabled={scanned != null} onClick={create}>
          {scanned != null ? <Spinner size={12} /> : null}
          {t.createIndex}
        </Button>
        {scanned != null ? (
          <span className="text-xs tabular-nums">{ja.filter.indexBuilding(scanned)}</span>
        ) : null}
      </div>
    </div>
  );
}

function DailyChart({ cost }: { cost: CostSummary }) {
  const values = cost.daily;
  const known = values.filter((v): v is number => v != null);
  const max = Math.max(0.0001, ...known);
  const mm = Number(cost.month.split('-')[1]);
  const days = values.length;
  const ticks = [1, 8, 15, 22, days];
  return (
    <>
      <div className="flex h-30 items-end gap-0.75" role="img" aria-label={t.daily}>
        {values.map((v, i) => (
          <Tooltip
            // biome-ignore lint/suspicious/noArrayIndexKey: 日ごとの固定の並び
            key={i}
            label={v != null ? `${mm}/${i + 1} ${formatUsd(v)}` : `${mm}/${i + 1}`}
            side="top"
          >
            <div
              className={cn(
                'flex-1 rounded-[2px]',
                v != null ? 'bg-primary' : 'shadow-[inset_0_0_0_1px_var(--border)]',
              )}
              style={{ height: `${Math.max(2, ((v ?? max * 0.9) / max) * 100)}%` }}
            />
          </Tooltip>
        ))}
      </div>
      <div className="mt-1.5 flex justify-between text-xs text-muted-foreground tabular-nums">
        {ticks.map((d) => (
          <span key={d}>{`${mm}/${d}`}</span>
        ))}
      </div>
    </>
  );
}

export function Dashboard({ connection }: { connection: Connection }) {
  const metrics = useStorageMetrics(connection.id);
  const bucket = useBucketInfo(connection.id);
  const cost = useCost(connection.id);
  const pricing = usePricing(connection.region);
  const settings = useSettings();
  const ceEnabled = settings.data?.cost.useCostExplorer ?? true;
  const m = metrics.data;
  const c = ceEnabled ? (cost.data ?? null) : null;
  const stale = c && c.month !== currentMonth() ? c.month : null;

  const rows = CLASS_ORDER.map((k) => {
    const bytes = m?.byClass[k] ?? 0;
    const price = pricing.data?.perGbMonth[k] ?? null;
    return { k, bytes, price, monthly: price != null ? (bytes / GB) * price : null };
  }).filter((r) => r.bytes > 0);
  const total = rows.reduce((s, r) => s + r.bytes, 0);

  const delta =
    c && c.prevMonthSamePeriod != null && c.prevMonthSamePeriod > 0
      ? (c.monthToDate - c.prevMonthSamePeriod) / c.prevMonthSamePeriod
      : null;
  const versioning = bucket.data?.versioning;
  const versioningOn = versioning === 'enabled';
  const breakdown = c
    ? ([
        [t.storage, c.breakdown.storage],
        [t.requests, c.breakdown.requests],
        [t.transfer, c.breakdown.transfer],
        [t.retrieval, c.breakdown.retrieval],
        ...(c.breakdown.other !== 0 ? [[t.other, c.breakdown.other] as const] : []),
      ] as const)
    : [];
  const priceNote =
    pricing.data?.source === 'bundled'
      ? t.unitPriceBundled(pricing.data.asOf)
      : t.unitPriceNote(connection.regionShort);

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div className="flex max-w-[1000px] flex-col gap-4 p-6">
        <div className="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
          <Kpi
            icon={HardDrive}
            label={t.usage}
            loading={metrics.isPending}
            value={m && m.source !== 'none' ? formatSize(m.totalBytes) : ja.common.none}
            sub={t.objects(m?.objectCount ?? null)}
          />
          <Kpi
            icon={Receipt}
            label={stale ? `${t.costThisMonth}（${monthLabel(stale)}）` : t.costThisMonth}
            loading={cost.isPending && ceEnabled}
            value={c ? formatUsd(c.monthToDate) : ja.common.none}
            sub={
              delta != null ? (
                <span>
                  {t.vsLastMonth}{' '}
                  <span className={cn('font-medium', delta > 0 ? 'text-destructive' : 'text-success')}>
                    {formatDelta(delta)}
                  </span>
                </span>
              ) : c ? (
                ja.common.none
              ) : ceEnabled ? (
                t.notFetched
              ) : (
                t.disabled
              )
            }
          />
          <Kpi icon={Globe} label={t.region} value={connection.regionShort} sub={connection.region} />
          <Kpi
            icon={History}
            label={t.versioning}
            loading={bucket.isPending}
            value={versioning ? ja.versioning[versioning] : ja.common.none}
            sub={versioningOn ? t.versioningOn : t.versioningOff}
          />
        </div>

        <Card>
          <CardHead right={<span className="text-sm text-muted-foreground">{priceNote}</span>}>
            {t.byClass}
          </CardHead>
          {metrics.isPending ? (
            <Skeleton className="h-24" />
          ) : m?.source === 'none' ? (
            <UsageFallback connection={connection} />
          ) : rows.length === 0 ? (
            <p className="m-0 text-sm text-muted-foreground">{m ? t.noObjects : t.noMetrics}</p>
          ) : (
            <>
              <UsageBar byClass={m?.byClass ?? {}} height={10} label={t.byClass} />
              <table className="mt-3.5 w-full border-collapse text-sm">
                <thead>
                  <tr className="text-muted-foreground">
                    {t.columns.map((h, i) => (
                      <th
                        key={h}
                        className={cn('pb-1.5 font-medium hairline-b', i ? 'text-right' : 'text-left')}
                      >
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody className="tabular-nums">
                  {rows.map((r) => (
                    <tr key={r.k}>
                      <td className="py-1.75 hairline-b">
                        <StorageClassBadge value={r.k} short={false} plain />
                      </td>
                      <td className="py-1.75 text-right hairline-b">{formatSize(r.bytes)}</td>
                      <td className="py-1.75 text-right text-muted-foreground hairline-b">
                        {formatPercent(r.bytes / total)}
                      </td>
                      <td className="py-1.75 text-right text-muted-foreground hairline-b">
                        {r.price != null ? formatUsd(r.price) : ja.common.none}
                      </td>
                      <td className="py-1.75 text-right font-medium hairline-b">{formatUsd(r.monthly)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {m?.source === 'index' ? (
                <p className="m-0 mt-2 text-xs text-muted-foreground">{t.fromIndex}</p>
              ) : null}
            </>
          )}
        </Card>

        <div className="grid grid-cols-[repeat(auto-fit,minmax(300px,1fr))] gap-4">
          <Card>
            <CardHead>{t.breakdown(monthLabel(c?.month ?? currentMonth()))}</CardHead>
            {c ? (
              <div className="flex flex-col text-base">
                {breakdown.map(([label, value]) => (
                  <div key={label} className="flex justify-between py-1.75 hairline-b">
                    <span>{label}</span>
                    <span className="tabular-nums">{formatUsd(value)}</span>
                  </div>
                ))}
                <div className="flex justify-between pt-2.5 font-bold">
                  <span>{t.total}</span>
                  <span className="tabular-nums">{formatUsd(c.monthToDate)}</span>
                </div>
              </div>
            ) : cost.isPending && ceEnabled ? (
              <Skeleton className="h-32" />
            ) : (
              <CostPlaceholder connection={connection} />
            )}
          </Card>
          <Card>
            <CardHead
              right={
                c?.forecastMonthEnd != null ? (
                  <Badge variant="outline">{t.forecast(formatUsd(c.forecastMonthEnd))}</Badge>
                ) : null
              }
            >
              {t.daily}
            </CardHead>
            {c ? (
              <DailyChart cost={c} />
            ) : cost.isPending && ceEnabled ? (
              <Skeleton className="h-32" />
            ) : (
              <CostPlaceholder connection={connection} />
            )}
          </Card>
        </div>

        <footer className="flex flex-col gap-0.5 text-xs text-muted-foreground">
          <span>{t.footnote}</span>
          <span>{t.cloudwatchNote}</span>
          {c ? (
            <span>
              {c.scope.kind === 'tag'
                ? t.scopeTag(c.scope.key, c.scope.value)
                : t.scopeAccount(c.scope.region)}{' '}
              · {t.fetchedAt(formatDate(c.fetchedAt))}
            </span>
          ) : null}
          {stale ? <span>{t.staleMonth(monthLabel(stale))}</span> : null}
          {m?.asOf ? <span>{`CloudWatch: ${formatDate(m.asOf)}`}</span> : null}
        </footer>
      </div>
    </div>
  );
}
