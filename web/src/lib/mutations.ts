import { createMutation, useQueryClient } from '@tanstack/solid-query';
import {
  checkTarget,
  createTarget,
  deleteTarget,
  dryRunTarget,
  importTargets,
  notifyStatus,
  setTargetEnabled,
  testDiscordIntegration,
  testRendererIntegration,
  updateTarget,
} from './api';
import { opsQueryKey, targetDetailQueryKey, targetsQueryKey } from './queries';
import type {
  DryRunResponse,
  IntegrationTestResponse,
  NotifyStatusResponse,
  ReloadReport,
  RendererTestResponse,
  TargetInput,
  TargetStatus,
} from './types';
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
      qc.invalidateQueries({ queryKey: [...opsQueryKey] });
      addToast('Re-checked', 'success');
    },
    onError: (e: Error) => addToast(`Check failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createImportTargetsMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: (toml: string) => importTargets(toml),
    onSuccess: (r: ReloadReport) => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      qc.invalidateQueries({ queryKey: [...opsQueryKey] });
      const touched = r.added.length + r.changed.length;
      addToast(
        `Imported ${touched} watch${touched === 1 ? '' : 'es'} (+${r.added.length} new / ~${r.changed.length} updated)`,
        'success',
      );
    },
    onError: (e: Error) => addToast(`Import failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createNotifyMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: () => notifyStatus(),
    onSuccess: (r: NotifyStatusResponse) => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      qc.invalidateQueries({ queryKey: [...opsQueryKey] });
      addToast(r.summary || 'Report sent', 'success');
    },
    onError: (e: Error) => addToast(`Report failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createTestDiscordMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: () => testDiscordIntegration(),
    onSuccess: (r: IntegrationTestResponse) => {
      qc.invalidateQueries({ queryKey: [...opsQueryKey] });
      addToast(r.message, r.ok ? 'success' : 'error');
    },
    onError: (e: Error) => addToast(`Discord test failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createTestRendererMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: () => testRendererIntegration(),
    onSuccess: (r: RendererTestResponse) => {
      qc.invalidateQueries({ queryKey: [...opsQueryKey] });
      addToast(r.message, r.ok ? 'success' : 'error');
    },
    onError: (e: Error) => addToast(`Renderer test failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createAddTargetMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: (input: TargetInput) => createTarget(input),
    onSuccess: (t: TargetStatus) => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      qc.invalidateQueries({ queryKey: [...opsQueryKey] });
      addToast(`Added ${t.name}`, 'success');
    },
    onError: (e: Error) => addToast(`Add failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createUpdateTargetMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: ({ id, input }: { id: string; input: TargetInput }) => updateTarget(id, input),
    onSuccess: (t: TargetStatus) => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      qc.invalidateQueries({ queryKey: [...targetDetailQueryKey(t.target_id)] });
      qc.invalidateQueries({ queryKey: [...opsQueryKey] });
      addToast(`Updated ${t.name}`, 'success');
    },
    onError: (e: Error) => addToast(`Update failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createDryRunMutation() {
  return createMutation(() => ({
    mutationFn: (input: TargetInput & { target_id?: string }) => dryRunTarget(input),
    onSuccess: (result: DryRunResponse) => {
      addToast(result.error ? `Dry-run found a problem: ${result.error}` : 'Dry-run complete', result.error ? 'error' : 'success');
    },
    onError: (e: Error) => addToast(`Dry-run failed: ${describeApiError(e)}`, 'error'),
  }));
}

export function createDeleteTargetMutation() {
  const qc = useQueryClient();
  return createMutation(() => ({
    mutationFn: (id: string) => deleteTarget(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [...targetsQueryKey] });
      qc.invalidateQueries({ queryKey: [...opsQueryKey] });
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
      qc.invalidateQueries({ queryKey: [...targetDetailQueryKey(v.id)] });
      qc.invalidateQueries({ queryKey: [...opsQueryKey] });
      addToast(v.enabled ? 'Enabled' : 'Disabled', 'success');
    },
    onError: (e: Error) => addToast(`Update failed: ${describeApiError(e)}`, 'error'),
  }));
}
