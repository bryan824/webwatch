// web/src/lib/api/mutations.ts
import { createMutation, useQueryClient } from '@tanstack/svelte-query';
import { toast } from 'svelte-sonner';
import { checkTarget, notifyStatus, reloadTargets } from './client';
import { targetsQueryKey } from './queries';
import type { ReloadReport, NotifyStatusResponse, TargetStatus } from './types';

function describeApiError(e: unknown): string {
  return e instanceof Error ? e.message : 'Request failed';
}

export function createCheckNowMutation() {
  const qc = useQueryClient();
  return createMutation<TargetStatus, Error, string>(() => ({
    mutationFn: (id: string) => checkTarget(id),
    onSuccess: () => { qc.invalidateQueries({ queryKey: targetsQueryKey }); toast.success('Re-checked'); },
    onError: (e: Error) => toast.error(`Check failed: ${describeApiError(e)}`)
  }));
}

export function createReloadMutation() {
  const qc = useQueryClient();
  return createMutation<ReloadReport, Error, void>(() => ({
    mutationFn: () => reloadTargets(),
    onSuccess: (r: ReloadReport) => {
      qc.invalidateQueries({ queryKey: targetsQueryKey });
      toast.success(`Reloaded: +${r.added.length} / -${r.removed.length} / ~${r.changed.length}`);
    },
    onError: (e: Error) => toast.error(`Reload failed: ${describeApiError(e)}`)
  }));
}

export function createNotifyMutation() {
  const qc = useQueryClient();
  return createMutation<NotifyStatusResponse, Error, void>(() => ({
    mutationFn: () => notifyStatus(),
    onSuccess: (r: NotifyStatusResponse) => { qc.invalidateQueries({ queryKey: targetsQueryKey }); toast.success(r.summary || 'Report sent'); },
    onError: (e: Error) => toast.error(`Report failed: ${describeApiError(e)}`)
  }));
}
