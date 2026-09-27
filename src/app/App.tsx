// メインウィンドウ（SCR-01〜03）。画面の切り替えは useUiStore.view で行う（01 §5.2）。

import { useEffect, useRef } from 'react';
import { AppShell } from '@/components/ds/app-shell';
import { Spinner } from '@/components/ui/misc';
import { toast } from '@/components/ui/toaster';
import { Browser } from '@/features/browser/browser';
import { Sidebar } from '@/features/browser/sidebar';
import { BrowserToolbar } from '@/features/browser/toolbar';
import { checkForUpdate, useCommands } from '@/features/commands';
import { Dashboard, DashboardToolbar } from '@/features/dashboard/dashboard';
import { DialogHost } from '@/features/dialogs/dialog-host';
import { Inspector } from '@/features/inspector/inspector';
import { SignIn } from '@/features/signin/sign-in';
import { useTransferCenter } from '@/features/transfer-center';
import { ja } from '@/lib/i18n/ja';
import type { Connection } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { useNavStore } from '@/stores/nav';
import { useUiStore } from '@/stores/ui';
import {
  applyAppearance,
  useLocationPersistence,
  useMainEvents,
  useMenuSync,
  useSharedEvents,
} from './app-events';
import { layoutFor, useWindowWidth } from './hooks';
import { useConnections, useSession, useSettings } from './queries';

/** 起動時に一度だけ: 前回の場所と表示の設定を復元する（03 §2、01 §5.4）。 */
function useRestoreOnce(connections: Connection[] | undefined) {
  const settings = useSettings();
  const restored = useRef(false);
  const viewApplied = useRef(false);

  useEffect(() => {
    if (viewApplied.current || !settings.data) return;
    viewApplied.current = true;
    const { view } = settings.data;
    const ui = useUiStore.getState();
    ui.setViewMode(view.mode);
    ui.setSort(view.sort);
    ui.setInspectorVisible(view.inspector);
    applyAppearance(settings.data.general.appearance);
    if (settings.data.advanced.autoCheckUpdate) void checkForUpdate(true);
  }, [settings.data]);

  useEffect(() => {
    if (restored.current || !connections) return;
    const nav = useNavStore.getState();
    if (nav.connectionId && connections.some((c) => c.id === nav.connectionId)) {
      restored.current = true;
      return;
    }
    const [first] = connections;
    if (!first) return;
    restored.current = true;
    ipc.connections
      .lastLocation()
      .then((loc) => {
        if (loc && connections.some((c) => c.id === loc.connectionId)) {
          useNavStore.getState().openConnection(loc.connectionId, loc.prefix);
        } else {
          useNavStore.getState().openConnection(first.id);
        }
      })
      .catch(() => useNavStore.getState().openConnection(first.id));
  }, [connections]);

  // 開いていた接続が削除された場合は先頭の接続を開く
  const connectionId = useNavStore((s) => s.connectionId);
  useEffect(() => {
    if (!connections || !connectionId) return;
    if (!connections.some((c) => c.id === connectionId)) {
      const [first] = connections;
      if (first) useNavStore.getState().openConnection(first.id);
      else useNavStore.getState().reset();
    }
  }, [connections, connectionId]);
}

function useStartupInfo() {
  useEffect(() => {
    ipc.app
      .startupInfo()
      .then((info) => {
        if (info.dbRecreated) toast.show({ tone: 'warning', title: ja.toast.dbRecreated });
      })
      .catch(() => {});
  }, []);
}

export function App() {
  const session = useSession();
  const signedIn = Boolean(session.data);
  const connections = useConnections(signedIn);
  const settings = useSettings();
  const view = useUiStore((s) => s.view);
  const connectionId = useNavStore((s) => s.connectionId);
  const inspectorVisible = useUiStore((s) => s.inspectorVisible);
  const hasSelection = useUiStore((s) => s.selection.keys.length > 0);
  const layout = layoutFor(useWindowWidth());

  const list = connections.data;
  const needsSignIn = !session.isPending && (!signedIn || (list !== undefined && list.length === 0));
  const main = signedIn && list !== undefined && list.length > 0;

  useSharedEvents();
  useMainEvents();
  useStartupInfo();
  useRestoreOnce(main ? list : undefined);
  useTransferCenter(signedIn);
  useCommands(main);
  useMenuSync(settings.data, true);
  useLocationPersistence(main);

  // メニューの状態のため、サインイン画面の表示中は view を signin にする
  useEffect(() => {
    const ui = useUiStore.getState();
    if (needsSignIn && ui.view !== 'signin') ui.setView('signin');
    if (main && ui.view === 'signin') ui.setView('files');
  }, [needsSignIn, main]);

  if (session.isPending || (signedIn && connections.isPending)) {
    return (
      <div
        data-tauri-drag-region
        className="grid h-full place-items-center bg-background text-muted-foreground"
      >
        <Spinner size={20} />
      </div>
    );
  }
  if (!main) {
    return (
      <>
        <SignIn session={session.data ?? null} />
        {signedIn ? <DialogHost connection={null} /> : null}
      </>
    );
  }

  const connection = list.find((c) => c.id === connectionId) ?? null;
  const dashboard = view === 'dashboard';
  const showInspector =
    !dashboard && connection !== null && inspectorVisible && (layout === 'wide' || hasSelection);
  const inspector = showInspector && connection ? <Inspector connection={connection} /> : null;

  return (
    <>
      <AppShell
        sidebar={<Sidebar connections={list} session={session.data as NonNullable<typeof session.data>} />}
        toolbar={
          dashboard ? (
            <DashboardToolbar connections={list} connection={connection} />
          ) : connection ? (
            <BrowserToolbar connection={connection} narrow={layout !== 'wide'} />
          ) : null
        }
        inspector={layout === 'compact' ? undefined : (inspector ?? undefined)}
        overlayInspector={layout === 'compact' ? (inspector ?? undefined) : undefined}
      >
        {connection ? (
          dashboard ? (
            <Dashboard key={connection.id} connection={connection} />
          ) : (
            <Browser key={connection.id} connection={connection} narrow={layout !== 'wide'} />
          )
        ) : (
          <div className="grid flex-1 place-items-center">
            <Spinner />
          </div>
        )}
      </AppShell>
      <DialogHost connection={connection} />
    </>
  );
}
