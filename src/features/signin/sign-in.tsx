// SCR-01 サインイン（オンボーディング）。DS: ui_kits/s3-drive/SignIn.jsx。

import { useQueryClient } from '@tanstack/react-query';
import { Check, CircleCheck, Lock } from 'lucide-react';
import { useState } from 'react';
import { qk } from '@/app/query-keys';
import appIcon from '@/assets/app-icon.svg';
import { Icon } from '@/components/ds/icon';
import { Button } from '@/components/ui/button';
import { toast } from '@/components/ui/toaster';
import { ConnectionForm } from '@/features/connections/connection-form';
import { ja } from '@/lib/i18n/ja';
import type { UserSession } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { regionShort } from '@/lib/region';
import { cn } from '@/lib/utils';
import { useNavStore } from '@/stores/nav';
import { useUiStore } from '@/stores/ui';

const t = ja.signin;

function Step({ n, label, current }: { n: 1 | 2; label: string; current: 1 | 2 }) {
  const done = current > n;
  const active = current === n;
  return (
    <div
      className={cn(
        'flex items-center gap-1.5 text-sm',
        current >= n ? 'text-foreground' : 'text-muted-foreground',
        active && 'font-semibold',
      )}
      aria-current={active ? 'step' : undefined}
    >
      <span
        className={cn(
          'grid size-4.5 place-items-center rounded-full text-xs font-semibold',
          done
            ? 'bg-success text-white'
            : active
              ? 'bg-primary text-primary-foreground'
              : 'bg-muted text-muted-foreground',
        )}
      >
        {done ? <Icon icon={Check} size={11} strokeWidth={3} /> : n}
      </span>
      {label}
    </div>
  );
}

export function SignIn({ session }: { session: UserSession | null }) {
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const step: 1 | 2 = session ? 2 : 1;

  const signIn = async () => {
    setBusy(true);
    setError(null);
    try {
      const s = await ipc.auth.signIn();
      queryClient.setQueryData(qk.session, s);
    } catch (e) {
      const err = ipc.toAppError(e);
      if (err.code !== 'AUTH_CANCELED' && err.code !== 'CANCELED') setError(err.message);
    } finally {
      setBusy(false);
    }
  };

  const change = async () => {
    try {
      await ipc.auth.signOut();
    } finally {
      queryClient.setQueryData(qk.session, null);
    }
  };

  const connect = async (input: ipc.ConnectionInput) => {
    const c = await ipc.connections.create(input);
    await queryClient.invalidateQueries({ queryKey: qk.connections });
    if (c.region !== input.region) {
      toast.show({ title: ja.connection.regionCorrected(regionShort(c.region)) });
    }
    useNavStore.getState().openConnection(c.id);
    useUiStore.getState().setView('files');
  };

  return (
    <div data-tauri-drag-region className="grid h-full place-items-center overflow-auto bg-background p-6">
      <div className="flex w-[380px] max-w-full flex-col items-center gap-5">
        <img src={appIcon} width={88} height={88} alt="" draggable={false} />
        <div className="text-center">
          <h1 className="m-0 text-xl font-bold tracking-[-0.01em]">{t.title}</h1>
          <p className="m-0 mt-1 text-base text-muted-foreground">{t.subtitle}</p>
        </div>
        <div className="flex items-center gap-2.5">
          <Step n={1} label={t.stepAccount} current={step} />
          <span className="h-(--hairline) w-6 bg-border" />
          <Step n={2} label={t.stepBucket} current={step} />
        </div>
        {step === 1 ? (
          <div className="flex w-full flex-col gap-2.5">
            <Button size="lg" variant="outline" className="w-full" disabled={busy} onClick={signIn}>
              {busy ? t.googleBusy : t.google}
            </Button>
            {busy ? (
              <Button
                variant="link"
                size="sm"
                className="self-center"
                onClick={() => void ipc.auth.cancelSignIn()}
              >
                {ja.common.cancel}
              </Button>
            ) : null}
            {error ? (
              <div role="alert" className="text-center text-sm text-destructive">
                {error}
              </div>
            ) : null}
            <p className="m-0 text-center text-xs text-pretty text-muted-foreground">{t.hint}</p>
          </div>
        ) : (
          <div className="flex w-full flex-col gap-3">
            <div className="flex items-center gap-2 rounded-lg bg-muted px-2.5 py-2 text-sm">
              <Icon icon={CircleCheck} size={14} style={{ color: 'var(--success)' }} />
              <span className="min-w-0 flex-1 truncate">{t.signedInAs(session?.email ?? '')}</span>
              <Button variant="link" size="sm" onClick={change}>
                {t.change}
              </Button>
            </div>
            <ConnectionForm id="signin-connection" onSubmit={connect} onBusyChange={setBusy} />
            <Button type="submit" form="signin-connection" size="lg" className="mt-1 w-full" disabled={busy}>
              {busy ? t.connectBusy : ja.common.connect}
            </Button>
            <div className="flex items-center justify-center gap-1.5 text-xs text-muted-foreground">
              <Icon icon={Lock} size={12} />
              {t.keychainNote}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
