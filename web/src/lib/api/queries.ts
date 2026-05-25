// web/src/lib/api/queries.ts
import { createQuery } from '@tanstack/svelte-query';
import { getTargets } from './client';
import type { TargetStatus } from './types';

export const targetsQueryKey = ['targets'] as const;

export function createTargetsQuery() {
  return createQuery<TargetStatus[]>(() => ({
    queryKey: targetsQueryKey,
    queryFn: getTargets,
    refetchInterval: 30_000,
    refetchOnWindowFocus: true,
    staleTime: 10_000
  }));
}
