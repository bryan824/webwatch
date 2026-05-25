// web/src/lib/api/client.ts
import { get } from 'svelte/store';
import { token } from '../stores/token';
import type { HealthResponse, NotifyStatusResponse, ReloadReport, TargetStatus } from './types';

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
  get unauthorized() { return this.status === 401; }
}

export async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  const t = get(token);
  if (t) headers.set('Authorization', `Bearer ${t}`);

  const res = await fetch(path, { ...init, headers });
  const isJson = res.headers.get('content-type')?.includes('application/json');
  const body = isJson ? await res.json().catch(() => null) : null;

  if (!res.ok) {
    const message = (body && typeof body.error === 'string') ? body.error : `HTTP ${res.status}`;
    throw new ApiError(res.status, message);
  }
  return body as T;
}

export const getTargets = () => apiFetch<TargetStatus[]>('/targets');
export const getHealth = () => apiFetch<HealthResponse>('/health');
export const checkTarget = (id: string) =>
  apiFetch<TargetStatus>(`/targets/${encodeURIComponent(id)}/status`);
export const reloadTargets = () => apiFetch<ReloadReport>('/targets/reload', { method: 'POST' });
export const notifyStatus = () => apiFetch<NotifyStatusResponse>('/notify/status', { method: 'POST' });
