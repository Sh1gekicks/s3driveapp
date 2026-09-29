// サイドバー（03 §5.1）。DS: ui_kits/s3-drive/Sidebar.jsx。

import { ChartPie, ChevronsUpDown, Database, KeyRound, LogOut, Plus, Settings } from 'lucide-react';
import { useStorageMetrics } from '@/app/queries';
import { SIDEBAR_ID } from '@/components/ds/app-shell';
import { Icon } from '@/components/ds/icon';
import { ResizeHandle } from '@/components/ds/resize-handle';
import { SidebarItem, SidebarSection } from '@/components/ds/sidebar-item';
import { UsageBar } from '@/components/ds/usage-bar';
import { ContextMenu, DropdownMenu, type MenuEntry } from '@/components/ui/menu';
import { Skeleton } from '@/components/ui/misc';
import { persistView, signOut } from '@/features/actions';
import { formatSize } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import type { Connection, UserSession } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { useNavStore } from '@/stores/nav';
import { SIDEBAR_WIDTH, useUiStore } from '@/stores/ui';

const t = ja.sidebar;

function UsageMeter({ connection }: { connection: Connection }) {
  const metrics = useStorageMetrics(connection.id);
  const m = metrics.data;
  return (
    <div className="flex flex-col gap-1.5 px-4.5 pt-2.5 pb-3">
      <div className="flex items-baseline justify-between text-xs">
        {metrics.isPending ? (
          <Skeleton className="h-3 w-24" />
        ) : (
          <span className="font-semibold tabular-nums">
            {m && m.source !== 'none' ? t.used(formatSize(m.totalBytes)) : ja.common.none}
          </span>
        )}
        <span className="text-muted-foreground">{connection.regionShort}</span>
      </div>
      <UsageBar byClass={m?.byClass ?? {}} label={m ? t.used(formatSize(m.totalBytes)) : undefined} />
      <div className="text-xs text-muted-foreground tabular-nums">
        {metrics.isPending ? (
          <Skeleton className="h-3 w-20" />
        ) : m?.objectCount != null ? (
          t.objects(m.objectCount)
        ) : (
          ja.common.none
        )}
      </div>
    </div>
  );
}

function AccountButton({ session }: { session: UserSession }) {
  const items: MenuEntry[] = [
    { id: 'settings', label: ja.menu.settings, icon: Settings, shortcut: '⌘,' },
    { id: 'credentials', label: ja.menu.credentials, icon: KeyRound },
    { separator: true, id: 's1' },
    { id: 'signOut', label: ja.menu.signOut, icon: LogOut },
  ];
  const onSelect = (id: string) => {
    if (id === 'settings') void ipc.app.openSettings();
    if (id === 'credentials') useUiStore.getState().openDialog({ type: 'credentials' });
    if (id === 'signOut') void signOut();
  };
  return (
    <DropdownMenu
      items={items}
      onSelect={onSelect}
      side="top"
      align="start"
      trigger={
        <button
          type="button"
          className="mx-2.5 mb-2.5 flex h-10 items-center gap-2.5 rounded-md px-2 text-left outline-none hover:bg-sidebar-accent focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]"
        >
          <span className="grid size-6 shrink-0 place-items-center rounded-full bg-azure-500 text-xs font-semibold text-white">
            {session.initial}
          </span>
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="truncate text-sm font-medium">{session.name}</span>
            <span className="truncate text-xs text-muted-foreground">{session.email}</span>
          </span>
          <Icon icon={ChevronsUpDown} size={12} className="text-muted-foreground" />
        </button>
      }
    />
  );
}

/** サイドバーの右端の幅変更のつまみ（03 §3）。幅は表示の設定として保存する。 */
export function SidebarResizer() {
  const width = useUiStore((s) => s.sidebarWidth);
  return (
    <ResizeHandle
      label={t.resize}
      controls={SIDEBAR_ID}
      value={width}
      min={SIDEBAR_WIDTH.min}
      max={SIDEBAR_WIDTH.max}
      defaultValue={SIDEBAR_WIDTH.default}
      onChange={(w) => useUiStore.getState().setSidebarWidth(w)}
      onCommit={persistView}
      className="-right-1"
    />
  );
}

export function Sidebar({ connections, session }: { connections: Connection[]; session: UserSession }) {
  const connectionId = useNavStore((s) => s.connectionId);
  const view = useUiStore((s) => s.view);
  const current = connections.find((c) => c.id === connectionId) ?? null;

  const openConnection = (id: string) => {
    const ui = useUiStore.getState();
    if (id !== connectionId) {
      useNavStore.getState().openConnection(id);
      ui.clearSearch();
      ui.setSelection([]);
    }
    ui.setView('files');
  };

  const connectionMenu = (id: string): MenuEntry[] => [
    { id: `edit:${id}`, label: t.editConnection },
    { id: `credentials:${id}`, label: t.updateCredentials, icon: KeyRound },
    { separator: true, id: 's1' },
    { id: `delete:${id}`, label: t.deleteConnection, destructive: true },
  ];
  const onConnectionMenu = (value: string) => {
    const [action, id = ''] = value.split(':');
    const ui = useUiStore.getState();
    const c = connections.find((x) => x.id === id);
    if (action === 'edit') ui.openDialog({ type: 'editConnection', connectionId: id });
    if (action === 'credentials') ui.openDialog({ type: 'credentials', credentialId: c?.credentialId });
    if (action === 'delete') ui.openDialog({ type: 'deleteConnection', connectionId: id });
  };

  return (
    <nav aria-label="サイドバー" className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1 overflow-auto pb-2">
        <SidebarSection title={t.buckets}>
          {connections.map((c) => (
            <ContextMenu key={c.id} items={connectionMenu(c.id)} onSelect={onConnectionMenu}>
              <SidebarItem
                icon={Database}
                label={c.bucket}
                title={`${c.bucket}（${c.regionShort}）`}
                active={c.id === connectionId && view === 'files'}
                onClick={() => openConnection(c.id)}
              />
            </ContextMenu>
          ))}
          <SidebarItem
            icon={Plus}
            label={t.addBucket}
            iconColor="var(--muted-foreground)"
            onClick={() => useUiStore.getState().openDialog({ type: 'addBucket' })}
          />
        </SidebarSection>
        <SidebarSection title={t.manage}>
          <SidebarItem
            icon={ChartPie}
            label={t.dashboard}
            active={view === 'dashboard'}
            onClick={() => useUiStore.getState().setView('dashboard')}
          />
        </SidebarSection>
      </div>
      {current ? <UsageMeter connection={current} /> : null}
      <AccountButton session={session} />
    </nav>
  );
}
