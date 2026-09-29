// 接続の入力フォーム（SCR-01 ステップ 2、DLG-05、接続の編集）。検証エラーと接続確認のエラーは
// 該当する入力欄の下に表示する（03 §4.3）。

import { zodResolver } from '@hookform/resolvers/zod';
import { ChevronRight } from 'lucide-react';
import { useState } from 'react';
import { Controller, type FieldPath, useForm } from 'react-hook-form';
import { Icon } from '@/components/ds/icon';
import { Field } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { NativeSelect } from '@/components/ui/native-select';
import { SegmentedControl } from '@/components/ui/segmented-control';
import { ja } from '@/lib/i18n/ja';
import { type AppError, type ConnectionInput, type CredentialSummary, toAppError } from '@/lib/ipc';
import { DEFAULT_REGION, REGIONS } from '@/lib/region';
import { cn } from '@/lib/utils';
import { type ConnectionFormValues, connectionSchema } from '@/lib/validation';

const t = ja.connection;

export const EMPTY_CONNECTION: ConnectionFormValues = {
  credentialMode: 'new',
  credentialId: '',
  accessKeyId: '',
  secretAccessKey: '',
  roleArn: '',
  externalId: '',
  region: DEFAULT_REGION,
  bucket: '',
};

export function toConnectionInput(v: ConnectionFormValues): ConnectionInput {
  const input: ConnectionInput = {
    bucket: v.bucket.trim(),
    region: v.region,
    credential:
      v.credentialMode === 'new'
        ? { kind: 'new', accessKeyId: v.accessKeyId.trim(), secretAccessKey: v.secretAccessKey }
        : { kind: 'existing', credentialId: v.credentialId },
  };
  if (v.roleArn.trim()) input.roleArn = v.roleArn.trim();
  if (v.externalId.trim()) input.externalId = v.externalId.trim();
  return input;
}

/** 接続確認のエラーを入力欄に割り当てる（03 §4.3）。 */
export function errorField(e: AppError, mode: 'new' | 'existing'): FieldPath<ConnectionFormValues> | null {
  switch (e.code) {
    case 'CREDENTIALS_INVALID':
    case 'CREDENTIALS_EXPIRED':
      return mode === 'new' ? 'secretAccessKey' : 'credentialId';
    case 'ROLE_ASSUME_DENIED':
      return 'roleArn';
    case 'BUCKET_NOT_FOUND':
    case 'BUCKET_ACCESS_DENIED':
    case 'INVALID_NAME':
      return 'bucket';
    default:
      return null;
  }
}

export interface ConnectionFormProps {
  id: string;
  defaultValues?: Partial<ConnectionFormValues>;
  credentials?: CredentialSummary[];
  /** 送信。失敗したら例外を投げる（入力欄にエラーを表示する）。 */
  onSubmit: (input: ConnectionInput) => Promise<void>;
  onBusyChange?: (busy: boolean) => void;
  className?: string;
}

export function ConnectionForm({
  id,
  defaultValues,
  credentials = [],
  onSubmit,
  onBusyChange,
  className,
}: ConnectionFormProps) {
  const [formError, setFormError] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState(Boolean(defaultValues?.externalId));
  const form = useForm<ConnectionFormValues>({
    resolver: zodResolver(connectionSchema),
    defaultValues: { ...EMPTY_CONNECTION, ...defaultValues },
    mode: 'onSubmit',
    reValidateMode: 'onChange',
  });
  const { register, control, handleSubmit, setError, watch, formState } = form;
  const mode = watch('credentialMode');
  const hasExisting = credentials.length > 0;
  const selectedCredential = credentials.find((c) => c.id === watch('credentialId'));

  const submit = handleSubmit(async (values) => {
    setFormError(null);
    onBusyChange?.(true);
    try {
      await onSubmit(toConnectionInput(values));
    } catch (e) {
      const err = toAppError(e);
      const field = errorField(err, values.credentialMode);
      if (field) setError(field, { message: err.message }, { shouldFocus: true });
      else setFormError(err.message);
    } finally {
      onBusyChange?.(false);
    }
  });

  const err = (name: FieldPath<ConnectionFormValues>) =>
    formState.errors[name]?.message as string | undefined;

  return (
    <form id={id} noValidate onSubmit={submit} className={cn('flex w-full flex-col gap-3', className)}>
      {hasExisting ? (
        <Controller
          control={control}
          name="credentialMode"
          render={({ field }) => (
            <SegmentedControl
              block
              aria-label={t.credential}
              value={field.value}
              onChange={field.onChange}
              options={[
                { value: 'new', label: t.newKey },
                { value: 'existing', label: t.existingKey },
              ]}
            />
          )}
        />
      ) : null}
      {mode === 'existing' && hasExisting ? (
        <Field
          label={t.credential}
          error={err('credentialId')}
          hint={selectedCredential ? t.usedBy(selectedCredential.usedBy) : undefined}
        >
          {(p) => (
            <NativeSelect
              id={p.id}
              aria-describedby={p['aria-describedby']}
              {...register('credentialId')}
              aria-invalid={p.invalid || undefined}
              options={[
                { value: '', label: '—' },
                ...credentials.map((c) => ({ value: c.id, label: c.accessKeyIdMasked })),
              ]}
            />
          )}
        </Field>
      ) : (
        <>
          <Field label={t.accessKeyId} error={err('accessKeyId')}>
            {(p) => (
              <Input
                {...p}
                {...register('accessKeyId')}
                mono
                autoComplete="off"
                autoCapitalize="characters"
                placeholder="AKIA…"
              />
            )}
          </Field>
          <Field label={t.secretAccessKey} error={err('secretAccessKey')}>
            {(p) => <Input {...p} {...register('secretAccessKey')} type="password" autoComplete="off" />}
          </Field>
        </>
      )}
      <Field label={t.roleArn} hint={t.roleArnHint} error={err('roleArn')}>
        {(p) => (
          <Input
            {...p}
            {...register('roleArn')}
            mono
            autoComplete="off"
            placeholder="arn:aws:iam::123456789012:role/…"
          />
        )}
      </Field>
      <div className="grid grid-cols-2 gap-2.5">
        <Field label={t.region} error={err('region')}>
          {(p) => (
            <NativeSelect
              id={p.id}
              aria-describedby={p['aria-describedby']}
              {...register('region')}
              options={REGIONS.map((r) => ({ value: r.code, label: r.short }))}
            />
          )}
        </Field>
        <Field label={t.bucket} error={err('bucket')}>
          {(p) => <Input {...p} {...register('bucket')} autoComplete="off" />}
        </Field>
      </div>
      <button
        type="button"
        aria-expanded={advanced}
        onClick={() => setAdvanced((v) => !v)}
        className="flex items-center gap-1 self-start rounded-sm text-sm text-muted-foreground outline-none hover:text-foreground focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]"
      >
        <Icon icon={ChevronRight} size={12} className={cn('transition-transform', advanced && 'rotate-90')} />
        {t.advanced}
      </button>
      {advanced ? (
        <Field label={t.externalId} hint={t.externalIdHint} error={err('externalId')}>
          {(p) => <Input {...p} {...register('externalId')} mono autoComplete="off" />}
        </Field>
      ) : null}
      {formError ? (
        <div role="alert" className="text-sm text-destructive">
          {formError}
        </div>
      ) : null}
    </form>
  );
}
