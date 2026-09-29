// 表示中のダイアログ（useUiStore.dialog）を描画する。

import { Download, Info, LogOut } from 'lucide-react';
import * as React from 'react';
import { Button } from '@/components/ui/button';
import { Dialog } from '@/components/ui/dialog';
import { Progress } from '@/components/ui/progress';
import { toast } from '@/components/ui/toaster';
import { signOut } from '@/features/actions';
import { showError } from '@/features/errors';
import { ja } from '@/lib/i18n/ja';
import type { Connection } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { activeTransferCount, useTransferStore } from '@/stores/transfers';
import { useUiStore } from '@/stores/ui';
import {
  AddBucketDialog,
  CredentialsDialog,
  DeleteConnectionDialog,
  EditConnectionDialog,
} from './connection-dialogs';
import {
  ConflictDialog,
  DeleteDialog,
  DeleteVersionDialog,
  MoveDialog,
  NewFolderDialog,
  RenameDialog,
  RestoreDialog,
  StorageClassDialog,
} from './file-dialogs';

function DetailsDialog({ title, lines, onClose }: { title: string; lines: string[]; onClose: () => void }) {
  return (
    <Dialog
      open
      onClose={onClose}
      icon={Info}
      width={480}
      title={title}
      footer={
        <Button onClick={onClose} autoFocus>
          {ja.common.close}
        </Button>
      }
    >
      <ul className="m-0 max-h-60 list-none overflow-auto rounded-lg p-2 text-sm shadow-[inset_0_0_0_0.5px_var(--border)] selectable">
        {lines.filter(Boolean).map((line, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: 同じ文言が並ぶことがある
          <li key={i} className="py-0.5 break-all">
            {line}
          </li>
        ))}
      </ul>
    </Dialog>
  );
}

function UpdateDialog({
  version,
  notes,
  onClose,
}: {
  version: string;
  notes: string | null;
  onClose: () => void;
}) {
  const [progress, setProgress] = React.useState<number | null | undefined>(undefined);
  /** 転送中のため、再起動のしかたを確認している（08 §7）。 */
  const [confirming, setConfirming] = React.useState(false);
  const active = useTransferStore((s) => activeTransferCount(s.jobs));
  const u = ja.dialog.update;
  const install = (whenIdle: boolean) => {
    setConfirming(false);
    setProgress(null);
    ipc.app
      .installUpdate((e) => {
        if (e.event === 'progress')
          setProgress(e.data.total ? (e.data.downloaded / e.data.total) * 100 : null);
      }, whenIdle)
      .then(() => {
        // 転送の完了後に再起動する場合だけ戻る（すぐに再起動する場合は戻らない）
        if (!whenIdle) return;
        onClose();
        toast.show({ icon: Download, title: u.scheduled, description: u.scheduledDesc });
      })
      .catch((e) => {
        setProgress(undefined);
        showError(e, u.install);
      });
  };
  const busy = progress !== undefined;
  const footer = confirming ? (
    <>
      <Button variant="outline" onClick={() => setConfirming(false)}>
        {ja.common.cancel}
      </Button>
      <Button variant="outline" className="text-destructive" onClick={() => install(false)}>
        {u.restartNow}
      </Button>
      <Button onClick={() => install(true)} autoFocus>
        {u.afterTransfers}
      </Button>
    </>
  ) : (
    <>
      <Button variant="outline" onClick={onClose} disabled={busy}>
        {u.later}
      </Button>
      <Button onClick={() => (active > 0 ? setConfirming(true) : install(false))} disabled={busy}>
        {busy ? u.installing : u.install}
      </Button>
    </>
  );
  return (
    <Dialog
      open
      onClose={busy ? () => {} : onClose}
      icon={Download}
      width={confirming ? 480 : undefined}
      title={u.title(version)}
      footer={footer}
    >
      {confirming ? (
        <div role="alert" className="flex flex-col gap-1 text-sm">
          <p className="m-0">{u.transfersActive(active)}</p>
          <p className="m-0 text-muted-foreground">{u.transfersHint}</p>
        </div>
      ) : notes ? (
        <p className="m-0 max-h-40 overflow-auto text-sm whitespace-pre-wrap text-muted-foreground">
          {notes}
        </p>
      ) : null}
      {busy ? <Progress value={progress ?? null} aria-label={u.installing} /> : null}
    </Dialog>
  );
}

/** サインアウトの確認（転送中の場合だけ。04 §1.4）。 */
function SignOutDialog({ onClose }: { onClose: () => void }) {
  const active = useTransferStore((s) => activeTransferCount(s.jobs));
  const t = ja.dialog.signOut;
  return (
    <Dialog
      open
      alert
      onClose={onClose}
      icon={LogOut}
      tone="destructive"
      title={t.title}
      description={t.description(active)}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>
            {ja.common.cancel}
          </Button>
          <Button
            variant="destructive"
            autoFocus
            onClick={() => {
              onClose();
              void signOut(true);
            }}
          >
            {ja.menu.signOut}
          </Button>
        </>
      }
    />
  );
}

export function DialogHost({ connection }: { connection: Connection | null }) {
  const dialog = useUiStore((s) => s.dialog);
  const close = React.useCallback(() => useUiStore.getState().closeDialog(), []);
  if (!dialog) return null;
  switch (dialog.type) {
    case 'newFolder':
      return <NewFolderDialog onClose={close} />;
    case 'rename':
      return <RenameDialog item={dialog.item} onClose={close} />;
    case 'delete':
      return <DeleteDialog items={dialog.items} onClose={close} />;
    case 'move':
      return <MoveDialog items={dialog.items} onClose={close} />;
    case 'storageClass':
      return <StorageClassDialog items={dialog.items} connection={connection} onClose={close} />;
    case 'restore':
      return <RestoreDialog items={dialog.items} onClose={close} />;
    case 'conflict':
      return (
        <ConflictDialog conflicts={dialog.conflicts} versioned={dialog.versioned} resolve={dialog.resolve} />
      );
    case 'deleteVersion':
      return (
        <DeleteVersionDialog
          versionKey={dialog.key}
          versionId={dialog.versionId}
          date={dialog.date}
          size={dialog.size}
          isLatest={dialog.isLatest}
          onClose={close}
        />
      );
    case 'addBucket':
      return <AddBucketDialog onClose={close} />;
    case 'editConnection':
      return <EditConnectionDialog connectionId={dialog.connectionId} onClose={close} />;
    case 'deleteConnection':
      return <DeleteConnectionDialog connectionId={dialog.connectionId} onClose={close} />;
    case 'credentials':
      return <CredentialsDialog credentialId={dialog.credentialId} onClose={close} />;
    case 'update':
      return <UpdateDialog version={dialog.version} notes={dialog.notes} onClose={close} />;
    case 'signOut':
      return <SignOutDialog onClose={close} />;
    case 'details':
      return <DetailsDialog title={dialog.title} lines={dialog.lines} onClose={close} />;
  }
}
