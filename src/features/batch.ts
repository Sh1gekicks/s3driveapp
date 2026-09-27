// 一括操作（削除・移動・クラス変更など）の進捗と結果の表示（01 §6.2、03 §11）。

import type { QueryKey } from '@tanstack/react-query';
import type { LucideIcon } from 'lucide-react';
import { queryClient } from '@/app/query-client';
import { type ToastOptions, toast } from '@/components/ui/toaster';
import { ja } from '@/lib/i18n/ja';
import type { BatchEvent, BatchResult } from '@/lib/ipc';
import { useUiStore } from '@/stores/ui';
import { showError } from './errors';

/** この件数以上は進捗付きのトーストを出す。 */
const PROGRESS_TOAST_THRESHOLD = 20;

export interface BatchOptions {
  /** 「移動」「削除」などの動詞（エラーの文言に使う）。 */
  verb: string;
  icon: LucideIcon;
  /** 進行中のトーストのタイトル。 */
  runningTitle: string;
  start: (onEvent: (e: BatchEvent) => void) => Promise<string>;
  /** 成功したときのトースト。 */
  success: (result: BatchResult) => Pick<ToastOptions, 'title' | 'description'>;
  /** 完了後に無効化するクエリ。 */
  invalidate: QueryKey[];
}

/** 結果の要約（テスト用に分けている）。 */
export function summarize(result: BatchResult): 'success' | 'partial' | 'failed' | 'skipped' {
  const failed = result.failed.length;
  if (failed === 0) return result.succeeded === 0 && result.skipped.length > 0 ? 'skipped' : 'success';
  return result.succeeded === 0 && result.skipped.length === 0 ? 'failed' : 'partial';
}

function detailLines(result: BatchResult): string[] {
  return [
    ...result.failed.map((f) => `${f.key || '—'}: ${f.error.message}`),
    ...result.skipped.map((s) => `${s.key}: ${s.reason}`),
  ];
}

export function runBatch(o: BatchOptions): Promise<BatchResult | null> {
  return new Promise((resolve) => {
    let toastId: string | null = null;
    const onEvent = (e: BatchEvent) => {
      if (e.event === 'progress') {
        const { done, total } = e.data;
        if (toastId === null && (total === null || total >= PROGRESS_TOAST_THRESHOLD)) {
          toastId = toast.show({ icon: o.icon, title: o.runningTitle, progress: 0, persistent: true });
        }
        if (toastId !== null) {
          toast.update(toastId, {
            icon: o.icon,
            title: o.runningTitle,
            description: total ? `${done} / ${total}` : `${done}`,
            progress: total ? (done / total) * 100 : null,
            persistent: true,
          });
        }
        return;
      }
      if (toastId !== null) toast.close(toastId);
      const result = e.data.result;
      for (const key of o.invalidate) void queryClient.invalidateQueries({ queryKey: key });
      const kind = summarize(result);
      const total = result.succeeded + result.failed.length + result.skipped.length;
      const showDetails = () =>
        useUiStore
          .getState()
          .openDialog({ type: 'details', title: ja.common.details, lines: detailLines(result) });
      if (kind === 'success') {
        toast.show({ icon: o.icon, tone: 'success', ...o.success(result) });
      } else if (kind === 'failed' && result.failed.length === 1 && result.failed[0]) {
        showError(result.failed[0].error, o.verb);
      } else {
        const notDone = result.failed.length + result.skipped.length;
        toast.show({
          tone: 'warning',
          title: ja.toast.partialFailure(total, notDone, o.verb),
          actions: [{ label: ja.common.details, onClick: showDetails }],
          persistent: true,
        });
      }
      resolve(result);
    };
    o.start(onEvent).catch((e) => {
      if (toastId !== null) toast.close(toastId);
      showError(e, o.verb);
      resolve(null);
    });
  });
}
