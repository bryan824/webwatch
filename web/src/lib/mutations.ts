import { createMutation, useQueryClient } from '@tanstack/solid-query';
import {
  checkTarget,
  createTarget,
  deleteTarget,
  notifyStatus,
  reloadTargets,
  setTargetEnabled,
} from './api';
import { targetsQueryKey } from './queries';
import type { ReloadReport, NotifyStatusResponse, TargetInput, TargetStatus } from './types';
import { addToast } from '../components/Toast';

function describeApiError(e: unknown): string {
  return e instanceof Error ? e.message : 'Request failed';
}

export function createCheckNowMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: (id: string) => checkTarget(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      addToast('Re-checked', 'success');
    },
    onError: (e: Error) => addToast(`Check failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createReloadMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: () => reloadTargets(),
    onSuccess: (r: ReloadReport) => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      addToast(`Reloaded: +${r.added.length} / -${r.removed.length} / ~${r.changed.length}`, 'success');
    },
    onError: (e: Error) => addToast(`Reload failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createNotifyMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: () => notifyStatus(),
    onSuccess: (r: NotifyStatusResponse) => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      addToast(r.summary || 'Report sent', 'success');
    },
    onError: (e: Error) => addToast(`Report failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createAddTargetMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: (input: TargetInput) => createTarget(input),
    onSuccess: (t: TargetStatus) => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      addToast(`Added ${t.name}`, 'success');
    },
    onError: (e: Error) => addToast(`Add failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createDeleteTargetMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: (id: string) => deleteTarget(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      addToast('Deleted', 'success');
    },
    onError: (e: Error) => addToast(`Delete failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createSetEnabledMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      setTargetEnabled(id, enabled),
    onSuccess: (_t: TargetStatus, v: { id: string; enabled: boolean }) => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      addToast(v.enabled ? 'Enabled' : 'Disabled', 'success');
    },
    onError: (e: Error) => addToast(`Update failed: ${describeApiError(e)}`, 'error'),
  }));
}
