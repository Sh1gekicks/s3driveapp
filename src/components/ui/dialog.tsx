import { AlertDialog } from '@base-ui/react/alert-dialog';
import { Dialog as BaseDialog } from '@base-ui/react/dialog';
import type { LucideIcon } from 'lucide-react';
import type * as React from 'react';
import { Icon } from '@/components/ds/icon';
import { cn } from '@/lib/utils';

// DS: components/feedback/Dialog.jsx。ウィンドウ内のオーバーレイ（--overlay）、角丸 12、余白 20、
// 左上に色付きタイルのアイコン、主ボタンは右端。Return で主ボタン、Esc でキャンセル（03 §8）。
// 確認は ui/alert-dialog 相当（role="alertdialog"）、入力は ui/dialog を使う（02 §6）。

export interface DialogProps {
  open: boolean;
  onClose: () => void;
  icon?: LucideIcon;
  tone?: 'default' | 'destructive';
  title: React.ReactNode;
  description?: React.ReactNode;
  width?: number;
  /** 確認のダイアログ（alertdialog）。外側のクリックでは閉じない。 */
  alert?: boolean;
  /** Return で実行する処理。指定するとフォームとして扱い、主ボタンを type="submit" にする。 */
  onSubmit?: () => void;
  footer?: React.ReactNode;
  children?: React.ReactNode;
}

export function Dialog({
  open,
  onClose,
  icon,
  tone = 'default',
  title,
  description,
  width = 420,
  alert,
  onSubmit,
  footer,
  children,
}: DialogProps) {
  const Parts = alert ? AlertDialog : BaseDialog;
  const color = tone === 'destructive' ? 'var(--destructive)' : 'var(--primary)';
  const content = (
    <>
      {children ? <div className="mt-4 flex flex-col gap-3">{children}</div> : null}
      {footer ? <div className="mt-5 flex justify-end gap-2">{footer}</div> : null}
    </>
  );
  return (
    <Parts.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <Parts.Portal>
        <Parts.Backdrop className="fixed inset-0 z-40 bg-overlay transition-opacity duration-(--dur-base) data-ending-style:opacity-0 data-starting-style:opacity-0" />
        <div className="pointer-events-none fixed inset-0 z-40 grid place-items-center">
          <Parts.Popup
            className={cn(
              'pointer-events-auto max-h-[calc(100%-40px)] max-w-[calc(100%-40px)] overflow-auto rounded-2xl bg-card p-5 text-card-foreground elevation-lg outline-none',
              'transition-[opacity,transform] duration-(--dur-base) ease-(--ease-out) data-ending-style:scale-[0.98] data-ending-style:opacity-0 data-starting-style:scale-[0.98] data-starting-style:opacity-0',
            )}
            style={{ width }}
          >
            <div className="flex gap-3">
              {icon ? (
                <div
                  className="flex size-9 shrink-0 items-center justify-center rounded-lg"
                  style={{ color, background: `color-mix(in oklch, ${color} 12%, transparent)` }}
                >
                  <Icon icon={icon} size={18} />
                </div>
              ) : null}
              <div className="min-w-0 flex-1">
                <Parts.Title className="m-0 text-md leading-[1.3] font-semibold break-words">
                  {title}
                </Parts.Title>
                {description ? (
                  <Parts.Description className="m-0 mt-1 text-base text-pretty text-muted-foreground">
                    {description}
                  </Parts.Description>
                ) : null}
              </div>
            </div>
            {onSubmit ? (
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  onSubmit();
                }}
              >
                {content}
              </form>
            ) : (
              content
            )}
          </Parts.Popup>
        </div>
      </Parts.Portal>
    </Parts.Root>
  );
}
