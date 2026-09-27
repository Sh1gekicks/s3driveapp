// 操作の失敗の知らせ方（01 §7.1、03 §11）。

import { KeyRound } from 'lucide-react';
import { queryClient } from '@/app/query-client';
import { qk } from '@/app/query-keys';
import { toast } from '@/components/ui/toaster';
import { ja } from '@/lib/i18n/ja';
import { type AppError, toAppError } from '@/lib/ipc';
import { useUiStore } from '@/stores/ui';

const CREDENTIAL_ERRORS = new Set(['CREDENTIALS_INVALID', 'CREDENTIALS_EXPIRED', 'ROLE_ASSUME_DENIED']);

export function isCredentialError(e: AppError): boolean {
  return CREDENTIAL_ERRORS.has(e.code);
}

/** 「{操作}できませんでした」のトースト。認証エラーは「更新…」、再試行できるものは「再試行」を付ける。 */
export function showError(e: unknown, verb: string, retry?: () => void) {
  const err = toAppError(e);
  if (err.code === 'CANCELED') return;
  if (err.code === 'AUTH_REQUIRED') {
    // サインインが必要: SCR-01 に戻す
    void queryClient.invalidateQueries({ queryKey: qk.session });
  }
  const actions = [];
  if (isCredentialError(err)) {
    actions.push({
      label: ja.toast.credentialsAction,
      onClick: () => useUiStore.getState().openDialog({ type: 'credentials' }),
    });
  } else if (err.retryable && retry) {
    actions.push({ label: ja.common.retry, onClick: retry });
  } else if (err.detail) {
    actions.push({
      label: ja.common.details,
      onClick: () =>
        useUiStore.getState().openDialog({
          type: 'details',
          title: ja.toast.failed(verb),
          lines: [err.message, err.detail ?? ''],
        }),
    });
  }
  toast.show({
    tone: 'destructive',
    icon: isCredentialError(err) ? KeyRound : undefined,
    title: ja.toast.failed(verb),
    description: err.message,
    actions,
    persistent: true,
  });
}
