import type {
  CheckRun,
  DryRunResponse,
  HealthResponse,
  IntegrationTestResponse,
  NotifyStatusResponse,
  OpsResponse,
  ReloadReport,
  RendererTestResponse,
  TargetInput,
  TargetStatus,
  WatchDetailResponse,
} from './types';

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

export async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(path, init);
  const isJson = res.headers.get('content-type')?.includes('application/json');
  const body = isJson ? await res.json().catch(() => null) : null;

  if (!res.ok) {
    const message = body && typeof body.error === 'string' ? body.error : `HTTP ${res.status}`;
    throw new ApiError(res.status, message);
  }
  return body as T;
}

export const getTargets = () => apiFetch<TargetStatus[]>('/targets');

export const getTargetDetail = (id: string) =>
  apiFetch<WatchDetailResponse>(`/targets/${encodeURIComponent(id)}`);

export const getTargetChecks = (id: string) =>
  apiFetch<CheckRun[]>(`/targets/${encodeURIComponent(id)}/checks`);

export const getHealth = () => apiFetch<HealthResponse>('/health');

export const getOps = () => apiFetch<OpsResponse>('/ops');

export const testDiscordIntegration = () =>
  apiFetch<IntegrationTestResponse>('/ops/discord/test', { method: 'POST' });

export const testRendererIntegration = () =>
  apiFetch<RendererTestResponse>('/ops/renderer/test', { method: 'POST' });

export const checkTarget = (id: string) =>
  apiFetch<TargetStatus>(`/targets/${encodeURIComponent(id)}/status`);

export const importTargets = (toml: string) =>
  apiFetch<ReloadReport>('/targets/import', {
    method: 'POST',
    headers: { 'content-type': 'application/toml' },
    body: toml,
  });

export const notifyStatus = () =>
  apiFetch<NotifyStatusResponse>('/notify/status', { method: 'POST' });

export const createTarget = (input: TargetInput) =>
  apiFetch<TargetStatus>('/targets', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  });

export const updateTarget = (id: string, input: TargetInput) =>
  apiFetch<TargetStatus>(`/targets/${encodeURIComponent(id)}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  });

export const dryRunTarget = (input: TargetInput & { target_id?: string }) =>
  apiFetch<DryRunResponse>('/targets/dry-run', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  });

export const deleteTarget = (id: string) =>
  apiFetch<void>(`/targets/${encodeURIComponent(id)}`, { method: 'DELETE' });

export const setTargetEnabled = (id: string, enabled: boolean) =>
  apiFetch<TargetStatus>(`/targets/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ enabled }),
  });
