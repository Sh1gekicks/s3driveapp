import * as React from 'react';
import { cn } from '@/lib/utils';

// DS: .s3-formrow。ラベルは上、ヒント・エラーは下（02 §6）。

export interface FieldProps {
  label?: React.ReactNode;
  hint?: React.ReactNode;
  error?: React.ReactNode;
  className?: string;
  /** 子の入力要素に id・aria-describedby を渡す。 */
  children: (props: { id: string; 'aria-describedby'?: string; invalid: boolean }) => React.ReactNode;
}

export function Field({ label, hint, error, className, children }: FieldProps) {
  const id = React.useId();
  const descId = `${id}-desc`;
  const message = error || hint;
  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      {label ? (
        <label htmlFor={id} className="text-sm font-medium text-foreground">
          {label}
        </label>
      ) : null}
      {children({ id, 'aria-describedby': message ? descId : undefined, invalid: Boolean(error) })}
      {message ? (
        <div
          id={descId}
          role={error ? 'alert' : undefined}
          className={cn('text-xs text-muted-foreground', error && 'text-destructive')}
        >
          {message}
        </div>
      ) : null}
    </div>
  );
}
