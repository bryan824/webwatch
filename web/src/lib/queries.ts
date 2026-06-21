import { createQuery } from '@tanstack/solid-query';
import { getOps, getTargetChecks, getTargetDetail, getTargets } from './api';
import type { CheckRun, OpsResponse, TargetStatus, WatchDetailResponse } from './types';

export const targetsQueryKey = ['targets'] as const;
export const opsQueryKey = ['ops'] as const;
export const targetDetailQueryKey = (id: string) => ['targets', id, 'detail'] as const;
export const targetChecksQueryKey = (id: string) => ['targets', id, 'checks'] as const;

export function createTargetsQuery() {
  return createQuery<TargetStatus[]>(() => ({
    queryKey: [...targetsQueryKey],
    queryFn: getTargets,
    refetchInterval: 30_000,
    refetchOnWindowFocus: true,
    staleTime: 10_000,
  }));
}

export function createOpsQuery() {
  return createQuery<OpsResponse>(() => ({
    queryKey: [...opsQueryKey],
    queryFn: getOps,
    refetchInterval: 30_000,
    refetchOnWindowFocus: true,
    staleTime: 10_000,
  }));
}

export function createTargetDetailQuery(id: () => string | undefined) {
  return createQuery<WatchDetailResponse>(() => ({
    queryKey: id() ? [...targetDetailQueryKey(id()!)] : ['targets', 'missing', 'detail'],
    queryFn: () => getTargetDetail(id()!),
    enabled: !!id(),
    staleTime: 10_000,
  }));
}

export function createTargetChecksQuery(id: () => string | undefined) {
  return createQuery<CheckRun[]>(() => ({
    queryKey: id() ? [...targetChecksQueryKey(id()!)] : ['targets', 'missing', 'checks'],
    queryFn: () => getTargetChecks(id()!),
    enabled: !!id(),
    refetchInterval: 30_000,
    staleTime: 10_000,
  }));
}
