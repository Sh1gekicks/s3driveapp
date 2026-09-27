// 両ウィンドウ共通の起動処理（プロバイダ、モックの差し込み、ウィンドウの表示、エラーの記録）。

import { QueryClientProvider } from '@tanstack/react-query';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { ToastProvider } from '@/components/ui/toaster';
import { TooltipProvider } from '@/components/ui/tooltip';
import { IS_MOCK, isNative } from '@/lib/platform';
import { queryClient } from './query-client';
import '@/styles/globals.css';
import { requireMock } from '@/lib/ipc/mock-loader';

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <ToastProvider>{children}</ToastProvider>
      </TooltipProvider>
    </QueryClientProvider>
  );
}

/** 想定外のエラーをログファイルに残す（01 §7.3）。 */
function forwardErrors() {
  const send = (message: string) => {
    void import('@tauri-apps/plugin-log').then((log) => log.error(message)).catch(() => {});
  };
  window.addEventListener('error', (e) => send(`[webview] ${e.message} (${e.filename}:${e.lineno})`));
  window.addEventListener('unhandledrejection', (e) =>
    send(`[webview] unhandled rejection: ${String(e.reason)}`),
  );
}

export async function bootstrap(element: React.ReactNode) {
  if (IS_MOCK) {
    // `pnpm dev:mock`: ?signedOut=1 でサインイン前、?empty=1 で接続なしから始める
    const params = new URLSearchParams(location.search);
    const { installMock } = await requireMock();
    installMock({ signedOut: params.has('signedOut'), noConnections: params.has('empty') });
  }
  const native = isNative();
  document.documentElement.dataset.platform = native ? 'tauri' : 'web';
  if (native) forwardErrors();

  const root = document.getElementById('root');
  if (!root) throw new Error('#root がありません');
  createRoot(root).render(
    <StrictMode>
      <Providers>{element}</Providers>
    </StrictMode>,
  );

  // ウィンドウは visible: false で作られる。最初の描画後に表示してちらつきを防ぐ（02 §7.4）
  if (native) {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        void import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().show());
      });
    });
  }
}
