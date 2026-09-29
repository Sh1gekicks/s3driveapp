// 接続・認証情報のダイアログ（DLG-05、DLG-06、接続の編集・削除）。

import { zodResolver } from '@hookform/resolvers/zod';
import { useQueryClient } from '@tanstack/react-query';
import { Database, KeyRound, Trash2 } from 'lucide-react';
import * as React from 'react';
import { useForm } from 'react-hook-form';
import { useConnections, useCredentials } from '@/app/queries';
import { qk } from '@/app/query-keys';
import { Button } from '@/components/ui/button';
import { Dialog } from '@/components/ui/dialog';
import { Field } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/misc';
import { toast } from '@/components/ui/toaster';
import { ConnectionForm } from '@/features/connections/connection-form';
import { showError } from '@/features/errors';
import { ja } from '@/lib/i18n/ja';
import * as ipc from '@/lib/ipc';
import { regionShort } from '@/lib/region';
import { type CredentialFormValues, credentialSchema } from '@/lib/validation';
import { useNavStore } from '@/stores/nav';
import { useUiStore } from '@/stores/ui';

interface Base {
  onClose: () => void;
}

function Footer({
  form,
  busy,
  label,
  busyLabel,
  onClose,
}: {
  form: string;
  busy: boolean;
  label: string;
  busyLabel?: string;
  onClose: () => void;
}) {
  return (
    <>
      <Button variant="outline" onClick={onClose}>
        {ja.common.cancel}
      </Button>
      <Button type="submit" form={form} disabled={busy}>
        {busy ? <Spinner /> : null}
        {busy && busyLabel ? busyLabel : label}
      </Button>
    </>
  );
}

// ---- DLG-05 バケットを追加 ---------------------------------------------------------------

export function AddBucketDialog({ onClose, openAfter = true }: Base & { openAfter?: boolean }) {
  const queryClient = useQueryClient();
  const credentials = useCredentials();
  const [busy, setBusy] = React.useState(false);
  const submit = async (input: ipc.ConnectionInput) => {
    const c = await ipc.connections.create(input);
    await queryClient.invalidateQueries({ queryKey: qk.connections });
    void queryClient.invalidateQueries({ queryKey: qk.credentials });
    if (c.region !== input.region)
      toast.show({ title: ja.connection.regionCorrected(regionShort(c.region)) });
    onClose();
    if (openAfter) {
      useNavStore.getState().openConnection(c.id);
      const ui = useUiStore.getState();
      ui.clearSearch();
      ui.setView('files');
    }
  };
  return (
    <Dialog
      open
      onClose={onClose}
      icon={Database}
      width={440}
      title={ja.dialog.addBucket.title}
      footer={
        <Footer
          form="add-bucket"
          busy={busy}
          label={ja.common.connect}
          busyLabel={ja.signin.connectBusy}
          onClose={onClose}
        />
      }
    >
      <ConnectionForm
        id="add-bucket"
        credentials={credentials.data ?? []}
        onSubmit={submit}
        onBusyChange={setBusy}
      />
    </Dialog>
  );
}

// ---- 接続を編集 ---------------------------------------------------------------------------

export function EditConnectionDialog({ connectionId, onClose }: Base & { connectionId: string }) {
  const queryClient = useQueryClient();
  const connections = useConnections();
  const credentials = useCredentials();
  const [busy, setBusy] = React.useState(false);
  const c = connections.data?.find((x) => x.id === connectionId);
  if (!c || !credentials.data) return null;
  const submit = async (input: ipc.ConnectionInput) => {
    await ipc.connections.update(connectionId, input);
    await queryClient.invalidateQueries({ queryKey: qk.connections });
    void queryClient.invalidateQueries({ queryKey: qk.credentials });
    void queryClient.invalidateQueries({ queryKey: qk.bucket(connectionId) });
    void queryClient.invalidateQueries({ queryKey: ['objects', connectionId] });
    onClose();
  };
  return (
    <Dialog
      open
      onClose={onClose}
      icon={Database}
      width={440}
      title={ja.dialog.editConnection.title}
      footer={
        <Footer
          form="edit-connection"
          busy={busy}
          label={ja.common.update}
          busyLabel={ja.signin.connectBusy}
          onClose={onClose}
        />
      }
    >
      <ConnectionForm
        id="edit-connection"
        credentials={credentials.data}
        defaultValues={{
          credentialMode: 'existing',
          credentialId: c.credentialId,
          roleArn: c.roleArn ?? '',
          externalId: c.externalId ?? '',
          region: c.region,
          bucket: c.bucket,
        }}
        onSubmit={submit}
        onBusyChange={setBusy}
      />
    </Dialog>
  );
}

// ---- 接続を削除 ---------------------------------------------------------------------------

export function DeleteConnectionDialog({ connectionId, onClose }: Base & { connectionId: string }) {
  const queryClient = useQueryClient();
  const connections = useConnections();
  const [busy, setBusy] = React.useState(false);
  const c = connections.data?.find((x) => x.id === connectionId);
  if (!c) return null;
  const submit = async () => {
    setBusy(true);
    try {
      await ipc.connections.remove(connectionId);
      const rest = (connections.data ?? []).filter((x) => x.id !== connectionId);
      queryClient.setQueryData(qk.connections, rest);
      void queryClient.invalidateQueries({ queryKey: qk.connections });
      void queryClient.invalidateQueries({ queryKey: qk.credentials });
      const nav = useNavStore.getState();
      if (nav.connectionId === connectionId) {
        const [next] = rest;
        if (next) nav.openConnection(next.id);
        else nav.reset();
      }
      toast.show({ tone: 'success', title: ja.dialog.deleteConnection.done });
      onClose();
    } catch (e) {
      showError(e, '接続を削除');
      setBusy(false);
    }
  };
  return (
    <Dialog
      open
      alert
      onClose={onClose}
      icon={Trash2}
      tone="destructive"
      title={ja.dialog.deleteConnection.title(c.bucket)}
      description={ja.dialog.deleteConnection.description}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>
            {ja.common.cancel}
          </Button>
          <Button variant="destructive" onClick={() => void submit()} disabled={busy} autoFocus>
            {ja.common.delete}
          </Button>
        </>
      }
    />
  );
}

// ---- DLG-06 認証情報を更新 ----------------------------------------------------------------

export function CredentialsDialog({ credentialId, onClose }: Base & { credentialId?: string }) {
  const queryClient = useQueryClient();
  const credentials = useCredentials();
  const connections = useConnections();
  const connId = useNavStore((s) => s.connectionId);
  const [formError, setFormError] = React.useState<string | null>(null);
  const targetId =
    credentialId ?? connections.data?.find((c) => c.id === connId)?.credentialId ?? credentials.data?.[0]?.id;
  const cred = credentials.data?.find((c) => c.id === targetId);
  const form = useForm<CredentialFormValues>({
    resolver: zodResolver(credentialSchema),
    defaultValues: { accessKeyId: '', secretAccessKey: '' },
  });
  const { register, handleSubmit, formState, setError } = form;
  const t = ja.connection;
  const submit = handleSubmit(async (v) => {
    if (!targetId) return;
    setFormError(null);
    try {
      await ipc.connections.updateCredential(targetId, v.accessKeyId.trim(), v.secretAccessKey);
      void queryClient.invalidateQueries({ queryKey: qk.credentials });
      void queryClient.invalidateQueries({ queryKey: qk.connections });
      // 認証エラーで止まっていた一覧を取り直す
      void queryClient.invalidateQueries({ queryKey: ['objects'] });
      toast.show({ tone: 'success', icon: KeyRound, title: ja.dialog.credentials.done });
      onClose();
    } catch (e) {
      const err = ipc.toAppError(e);
      if (err.code === 'CREDENTIALS_INVALID' || err.code === 'CREDENTIALS_EXPIRED') {
        setError('secretAccessKey', { message: err.message }, { shouldFocus: true });
      } else {
        setFormError(err.message);
      }
    }
  });
  return (
    <Dialog
      open
      onClose={onClose}
      icon={KeyRound}
      width={420}
      title={ja.dialog.credentials.title}
      description={cred ? t.usedBy(cred.usedBy) : undefined}
      footer={
        <Footer
          form="credentials"
          busy={formState.isSubmitting}
          label={ja.dialog.credentials.confirm}
          busyLabel={ja.signin.connectBusy}
          onClose={onClose}
        />
      }
    >
      <form id="credentials" noValidate onSubmit={submit} className="flex flex-col gap-3">
        <Field label={t.accessKeyId} error={formState.errors.accessKeyId?.message}>
          {(p) => (
            <Input
              {...p}
              {...register('accessKeyId')}
              mono
              autoFocus
              autoComplete="off"
              placeholder="AKIA…"
            />
          )}
        </Field>
        <Field label={t.secretAccessKey} error={formState.errors.secretAccessKey?.message}>
          {(p) => <Input {...p} {...register('secretAccessKey')} type="password" autoComplete="off" />}
        </Field>
        {formError ? (
          <div role="alert" className="text-sm text-destructive">
            {formError}
          </div>
        ) : null}
      </form>
    </Dialog>
  );
}
