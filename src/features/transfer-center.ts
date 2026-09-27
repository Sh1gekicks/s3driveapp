// 転送の進捗と完了の知らせ（03 §11）。転送チャネルを購読し、ジョブごとにトーストを 1 つ表示する。

import { Download, Upload } from 'lucide-react';
import { useEffect } from 'react';
import { queryClient } from '@/app/query-client';
import { qk } from '@/app/query-keys';
import { toast } from '@/components/ui/toaster';
import { formatEta, formatSize } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import type { FileFailed, Settings, TransferEvent, TransferJob } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { useTransferStore } from '@/stores/transfers';
import { useUiStore } from '@/stores/ui';
import { changedKeys } from './actions';
import { showError } from './errors';

const toastIds = new Map<string, string>();
const failures = new Map<string, FileFailed[]>();
const finished = new Set<string>();

function progressDescription(job: TransferJob): string {
  const base = ja.toast
    .transferProgress(job.currentName ?? '', formatSize(job.doneBytes), formatSize(job.totalBytes))
    .replace(/^ · /, '');
  const eta = formatEta(job.etaSec);
  return eta ? `${base} · ${eta}` : base;
}

function notifyIfHidden(title: string, body: string) {
  const settings = queryClient.getQueryData<Settings>(qk.settings);
  if (settings && !settings.transfer.notifyOnComplete) return;
  if (typeof document !== 'undefined' && document.hasFocus()) return;
  void ipc.app.notify(title, body).catch(() => {});
}

function onJob(job: TransferJob) {
  useTransferStore.getState().upsert(job);
  const icon = job.kind === 'upload' ? Upload : Download;
  const existing = toastIds.get(job.jobId);
  if (job.status === 'queued' || job.status === 'running') {
    const opts = {
      icon,
      title: job.title,
      description: progressDescription(job),
      progress: job.totalBytes > 0 ? (job.doneBytes / job.totalBytes) * 100 : null,
      persistent: true,
      actions: [
        { label: ja.common.cancel, onClick: () => void ipc.transfers.cancel(job.jobId).catch(() => {}) },
      ],
    };
    if (existing) toast.update(existing, opts);
    else toastIds.set(job.jobId, toast.show(opts));
    return;
  }
  if (finished.has(job.jobId)) return;
  finished.add(job.jobId);
  if (existing) {
    toast.close(existing);
    toastIds.delete(job.jobId);
  }
  for (const key of changedKeys(job.connectionId)) void queryClient.invalidateQueries({ queryKey: key });
  const failed = failures.get(job.jobId) ?? [];
  failures.delete(job.jobId);
  const verb = job.kind === 'upload' ? ja.verbs.upload : ja.verbs.download;
  const showDetails = () =>
    useUiStore.getState().openDialog({
      type: 'details',
      title: ja.common.details,
      lines: failed.map((f) => `${f.name}: ${f.error.message}`),
    });
  const retry = () => void ipc.transfers.retry(job.jobId).catch((e) => showError(e, verb));

  switch (job.status) {
    case 'succeeded': {
      if (job.failedFiles > 0) {
        toast.show({
          tone: 'warning',
          title: ja.toast.partialFailure(job.totalFiles, job.failedFiles, verb),
          actions: [
            { label: ja.common.details, onClick: showDetails },
            { label: ja.common.retry, onClick: retry },
          ],
          persistent: true,
        });
        return;
      }
      if (job.kind === 'upload') {
        const desc = ja.toast.uploadDoneDesc(job.doneFiles, job.destination);
        toast.show({ tone: 'success', title: ja.toast.uploadDone, description: desc });
        notifyIfHidden(ja.toast.uploadDone, desc);
      } else {
        const desc = ja.toast.downloadDoneDesc(job.destination);
        toast.show({
          tone: 'success',
          title: ja.toast.downloadDone,
          description: desc,
          actions: [
            { label: ja.toast.reveal, onClick: () => void ipc.transfers.reveal(job.jobId).catch(() => {}) },
          ],
        });
        notifyIfHidden(ja.toast.downloadDone, desc);
      }
      return;
    }
    case 'failed': {
      const first = failed[0];
      if (first && failed.length === 1) {
        showError(first.error, verb, retry);
      } else {
        toast.show({
          tone: 'destructive',
          title: ja.toast.failed(verb),
          description: first?.error.message,
          actions: failed.length
            ? [
                { label: ja.common.details, onClick: showDetails },
                { label: ja.common.retry, onClick: retry },
              ]
            : [{ label: ja.common.retry, onClick: retry }],
          persistent: true,
        });
      }
      return;
    }
    case 'canceled':
      toast.show({ title: ja.toast.canceled });
      return;
  }
}

function onEvent(e: TransferEvent) {
  if (e.event === 'jobUpdated') {
    onJob(e.data);
  } else {
    const list = failures.get(e.data.jobId) ?? [];
    list.push(e.data);
    failures.set(e.data.jobId, list);
  }
}

/** メインウィンドウで一度だけ購読する。 */
export function useTransferCenter(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    let active = true;
    ipc.transfers
      .subscribe((e) => {
        if (active) onEvent(e);
      })
      .then((jobs) => {
        if (!active) return;
        useTransferStore.getState().replace(jobs);
        for (const j of jobs) if (j.status === 'queued' || j.status === 'running') onJob(j);
        // 購読前に終わったジョブは知らせない
        for (const j of jobs) if (j.status !== 'queued' && j.status !== 'running') finished.add(j.jobId);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [enabled]);
}
