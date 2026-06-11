import type { TargetStatus } from './types';

export type StatusKind = 'matched' | 'no_match' | 'unknown' | 'error';

export interface DerivedStatus {
  kind: StatusKind;
  label: string;
  tone: 'green' | 'muted' | 'amber' | 'red';
}

function errorIsCurrent(t: TargetStatus): boolean {
  if (!t.last_error) return false;
  if (!t.last_success_at) return true;
  if (!t.last_error_at) return false;
  return new Date(t.last_error_at) >= new Date(t.last_success_at);
}

export function deriveStatus(t: TargetStatus): DerivedStatus {
  if (errorIsCurrent(t)) return { kind: 'error', label: 'Error', tone: 'red' };
  if (t.matched === true) return { kind: 'matched', label: 'Matched', tone: 'green' };
  if (t.matched === false) return { kind: 'no_match', label: 'No match', tone: 'muted' };
  return { kind: 'unknown', label: 'Unknown', tone: 'amber' };
}
