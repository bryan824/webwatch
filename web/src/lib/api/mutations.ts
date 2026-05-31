// web/src/lib/api/mutations.ts
import { createMutation, useQueryClient } from '@tanstack/svelte-query';
import { toast } from 'svelte-sonner';
import {
  checkTarget,
  createTarget,
  deleteTarget,
  notifyStatus,
  reloadTargets,
  setTargetEnabled
} from './client';
import { targetsQueryKey } from './queries';
import type { ReloadReport, NotifyStatusResponse, TargetInput, TargetStatus } from './types';

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

export function createAddTargetMutation() {
  const qc = useQueryClient();
  return createMutation<TargetStatus, Error, TargetInput>(() => ({
    mutationFn: (input: TargetInput) => createTarget(input),
    onSuccess: (t: TargetStatus) => { qc.invalidateQueries({ queryKey: targetsQueryKey }); toast.success(`Added ${t.name}`); },
    onError: (e: Error) => toast.error(`Add failed: ${describeApiError(e)}`)
  }));
}

export function createDeleteTargetMutation() {
  const qc = useQueryClient();
  return createMutation<void, Error, string>(() => ({
    mutationFn: (id: string) => deleteTarget(id),
    onSuccess: () => { qc.invalidateQueries({ queryKey: targetsQueryKey }); toast.success('Deleted'); },
    onError: (e: Error) => toast.error(`Delete failed: ${describeApiError(e)}`)
  }));
}

export function createSetEnabledMutation() {
  const qc = useQueryClient();
  return createMutation<TargetStatus, Error, { id: string; enabled: boolean }>(() => ({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) => setTargetEnabled(id, enabled),
    onSuccess: (_t: TargetStatus, v: { id: string; enabled: boolean }) => {
      qc.invalidateQueries({ queryKey: targetsQueryKey });
      toast.success(v.enabled ? 'Enabled' : 'Disabled');
    },
    onError: (e: Error) => toast.error(`Update failed: ${describeApiError(e)}`)
  }));
}
