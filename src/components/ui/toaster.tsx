import { Toast } from '@base-ui/react/toast';
import { CircleAlert, CircleCheck, Info, type LucideIcon, TriangleAlert, X } from 'lucide-react';
import type * as React from 'react';
import { Icon } from '@/components/ds/icon';
import { ja } from '@/lib/i18n/ja';
import { cn } from '@/lib/utils';
import { Button } from './button';
import { Progress } from './progress';

// DS: components/feedback/Toast.jsx。右下（16px 内側）に積み、間隔 8px、幅 340px。
// 同時に表示するのは 4 件までとし、それを超える分は「ほか {n} 件」にまとめる（03 §11）。

export type ToastTone = 'default' | 'success' | 'warning' | 'destructive';

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface ToastData {
  icon?: LucideIcon;
  tone?: ToastTone;
  /** 0〜100。null は不定。undefined は表示しない。 */
  progress?: number | null;
  actions?: ToastAction[];
}

export interface ToastOptions extends ToastData {
  id?: string;
  title: React.ReactNode;
  description?: React.ReactNode;
  /** 自動で閉じない（転送中・エラー・一部失敗）。 */
  persistent?: boolean;
}

/** 自動で閉じるまでの時間（03 §11）。 */
export const TOAST_TIMEOUT = 3200;
const MAX_VISIBLE = 4;

export const toastManager = Toast.createToastManager();

const TONE_COLOR: Record<ToastTone, string> = {
  default: 'var(--primary)',
  success: 'var(--success)',
  warning: 'var(--warning)',
  destructive: 'var(--destructive)',
};

const DEFAULT_ICON: Record<ToastTone, LucideIcon> = {
  default: Info,
  success: CircleCheck,
  warning: TriangleAlert,
  destructive: CircleAlert,
};

function toManager(o: ToastOptions) {
  const tone = o.tone ?? 'default';
  return {
    title: o.title,
    description: o.description,
    type: tone,
    timeout: o.persistent ? 0 : TOAST_TIMEOUT,
    // エラーは assertive、それ以外は polite で読み上げる（02 §10）
    priority: tone === 'destructive' ? ('high' as const) : ('low' as const),
    data: { icon: o.icon, tone, progress: o.progress, actions: o.actions } satisfies ToastData,
  };
}

export const toast = {
  show(o: ToastOptions): string {
    return toastManager.add({ ...toManager(o), id: o.id });
  },
  update(id: string, o: ToastOptions) {
    toastManager.update(id, toManager(o));
  },
  close(id: string) {
    toastManager.close(id);
  },
};

export function ToastProvider({ children }: { children: React.ReactNode }) {
  return (
    <Toast.Provider toastManager={toastManager} limit={MAX_VISIBLE} timeout={TOAST_TIMEOUT}>
      {children}
      <Toast.Portal>
        <Toast.Viewport className="fixed right-4 bottom-4 z-45 flex w-[340px] max-w-[calc(100vw-32px)] flex-col-reverse gap-2 outline-none">
          <ToastList />
        </Toast.Viewport>
      </Toast.Portal>
    </Toast.Provider>
  );
}

function ToastList() {
  const { toasts } = Toast.useToastManager();
  const hidden = toasts.filter((t) => t.limited).length;
  return (
    <>
      {toasts.map((t) => {
        const data = (t.data ?? {}) as ToastData;
        const tone = data.tone ?? 'default';
        return (
          <Toast.Root
            key={t.id}
            toast={t}
            data-tone={tone}
            className={cn(
              'material flex w-full items-start gap-2.5 rounded-xl p-3 text-popover-foreground elevation-md outline-none',
              'transition-[opacity,transform] duration-(--dur-base) ease-(--ease-out) data-ending-style:opacity-0 data-limited:hidden data-starting-style:translate-y-2 data-starting-style:opacity-0',
            )}
          >
            <Icon icon={data.icon ?? DEFAULT_ICON[tone]} size={18} style={{ color: TONE_COLOR[tone] }} />
            <Toast.Content className="flex min-w-0 flex-1 flex-col gap-0.75">
              <Toast.Title className="m-0 text-base font-semibold" />
              {t.description ? (
                <Toast.Description className="m-0 truncate text-sm text-muted-foreground" />
              ) : null}
              {data.progress !== undefined ? (
                <Progress value={data.progress} height={4} className="mt-1.25" aria-label="進捗" />
              ) : null}
              {data.actions?.length ? (
                <div className="mt-1 flex gap-3">
                  {data.actions.map((a) => (
                    <Button key={a.label} variant="link" size="sm" onClick={a.onClick}>
                      {a.label}
                    </Button>
                  ))}
                </div>
              ) : null}
            </Toast.Content>
            <Toast.Close
              aria-label={ja.common.close}
              className="flex size-6 shrink-0 items-center justify-center rounded-sm text-muted-foreground outline-none hover:bg-accent focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]"
            >
              <Icon icon={X} size={14} />
            </Toast.Close>
          </Toast.Root>
        );
      })}
      {hidden > 0 ? (
        <div className="material self-end rounded-full px-3 py-1 text-sm text-muted-foreground elevation-sm">
          {ja.toast.more(hidden)}
        </div>
      ) : null}
    </>
  );
}
