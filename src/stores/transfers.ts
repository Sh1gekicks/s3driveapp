// 転送ジョブの一覧と進捗（転送チャネルのイベントで更新する。01 §5.4）。

import { create } from 'zustand';
import type { TransferJob } from '@/lib/ipc';

export interface TransferState {
  jobs: Record<string, TransferJob>;
  /** 転送ジョブの一覧を置き換える（購読の開始時）。 */
  replace: (jobs: TransferJob[]) => void;
  upsert: (job: TransferJob) => void;
  remove: (jobId: string) => void;
}

export const useTransferStore = create<TransferState>((set) => ({
  jobs: {},
  replace: (jobs) => set({ jobs: Object.fromEntries(jobs.map((j) => [j.jobId, j])) }),
  upsert: (job) => set((s) => ({ jobs: { ...s.jobs, [job.jobId]: job } })),
  remove: (jobId) =>
    set((s) => {
      const { [jobId]: _, ...rest } = s.jobs;
      return { jobs: rest };
    }),
}));

export function isActive(job: TransferJob): boolean {
  return job.status === 'queued' || job.status === 'running';
}

/** 転送中（待機中を含む）のジョブの数（サインアウトとアップデートの適用の前に確認する。04 §1.4、08 §7）。 */
export function activeTransferCount(
  jobs: Record<string, TransferJob> = useTransferStore.getState().jobs,
): number {
  return Object.values(jobs).filter(isActive).length;
}
