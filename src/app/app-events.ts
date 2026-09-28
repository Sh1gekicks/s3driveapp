// アプリ全体のイベントの購読、外観の反映、メニューの状態の同期、場所の保存（01 §5、05 §4.2）。

import { useQueryClient } from '@tanstack/react-query';
import { ArchiveRestore } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { toast } from '@/components/ui/toaster';
import { menuState } from '@/features/commands';
import { useVisibleStore } from '@/features/context';
import { ja } from '@/lib/i18n/ja';
import type { Appearance, Settings } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { isNative } from '@/lib/platform';
import { useNavStore } from '@/stores/nav';
import { useUiStore } from '@/stores/ui';
import { qk } from './query-keys';

/** 外観（自動・ライト・ダーク）を data-theme に反映する（02 §4）。 */
export function applyAppearance(appearance: Appearance | undefined) {
  const root = document.documentElement;
  if (!appearance || appearance === 'auto') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', appearance);
}

/** 設定・サインイン状態・接続の変更を購読する（両方のウィンドウ）。 */
export function useSharedEvents() {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = [
      ipc.events.onSettings((s) => {
        queryClient.setQueryData(qk.settings, s);
        applyAppearance(s.general.appearance);
      }),
      ipc.events.onSession((s) => {
        queryClient.setQueryData(qk.session, s);
        if (!s) queryClient.removeQueries({ queryKey: qk.connections });
      }),
      ipc.events.onConnections(() => {
        void queryClient.invalidateQueries({ queryKey: qk.connections });
        void queryClient.invalidateQueries({ queryKey: qk.credentials });
      }),
    ];
    return () => {
      for (const u of unlisten) void u.then((f) => f()).catch(() => {});
    };
  }, [queryClient]);
}

/** メインウィンドウだけのイベント（取り出しの完了、インデックスの更新、アップデート）。 */
export function useMainEvents() {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = [
      ipc.events.onRestoreCompleted((p) => {
        for (const key of [
          ['objects', p.connectionId],
          ['object', p.connectionId],
          ['search', p.connectionId],
        ]) {
          void queryClient.invalidateQueries({ queryKey: key });
        }
        const name = p.key.slice(p.key.lastIndexOf('/') + 1);
        toast.show({
          icon: ArchiveRestore,
          tone: 'success',
          title: ja.dialog.restore.completed,
          description: ja.dialog.restore.completedDesc(name),
        });
        const settings = queryClient.getQueryData<Settings>(qk.settings);
        if (!document.hasFocus() && settings?.transfer.notifyOnComplete !== false) {
          void ipc.app
            .notify(ja.dialog.restore.completed, ja.dialog.restore.completedDesc(name))
            .catch(() => {});
        }
      }),
      ipc.events.onIndexUpdated((p) => {
        queryClient.setQueryData(qk.indexStatus(p.connectionId), p.status);
        void queryClient.invalidateQueries({ queryKey: qk.search(p.connectionId) });
        // CloudWatch のメトリクスがなければインデックスから集計しているため（04 §12.2）
        void queryClient.invalidateQueries({ queryKey: qk.storageMetrics(p.connectionId) });
      }),
      ipc.events.onUpdateAvailable((info) => {
        if (!useUiStore.getState().dialog) {
          useUiStore.getState().openDialog({ type: 'update', version: info.version, notes: info.notes });
        }
      }),
    ];
    return () => {
      for (const u of unlisten) void u.then((f) => f()).catch(() => {});
    };
  }, [queryClient]);
}

/** メニューバーの有効・無効を選択状態や画面に合わせる（03 §9.4）。 */
export function useMenuSync(settings: Settings | undefined, active: boolean) {
  const last = useRef('');
  useEffect(() => {
    if (!active || !isNative()) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const push = () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        const state = menuState(settings);
        const json = JSON.stringify(state);
        if (json === last.current) return;
        last.current = json;
        void ipc.app.updateMenuState(state).catch(() => {});
      }, 50);
    };
    push();
    const unsubs = [useUiStore.subscribe(push), useNavStore.subscribe(push), useVisibleStore.subscribe(push)];
    return () => {
      clearTimeout(timer);
      for (const u of unsubs) u();
    };
  }, [settings, active]);
}

/** 表示中の場所を保存し、次回の起動時に復元する（03 §2）。 */
export function useLocationPersistence(active: boolean) {
  useEffect(() => {
    if (!active) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let prev = '';
    const unsubscribe = useNavStore.subscribe((s) => {
      if (!s.connectionId) return;
      const key = `${s.connectionId}\n${s.prefix}`;
      if (key === prev) return;
      prev = key;
      clearTimeout(timer);
      const location = { connectionId: s.connectionId, prefix: s.prefix };
      timer = setTimeout(() => void ipc.connections.setLocation(location).catch(() => {}), 500);
    });
    // サインアウトなどで購読をやめたあとに、保留中の保存が遅れて届かないようにする
    return () => {
      unsubscribe();
      clearTimeout(timer);
    };
  }, [active]);
}
