// web/src/lib/status.test.ts
import { describe, it, expect } from 'vitest';
import { deriveStatus } from './status';
import type { TargetStatus } from './api/types';

const base: TargetStatus = {
  target_id: 'x', name: 'X', url: 'https://e.com', matched: null,
  engine_used: null, price_cents: null, evidence: [], condition_results: [],
  last_success_at: null, last_error_at: null, last_error: null, last_alert_at: null
};

describe('deriveStatus', () => {
  it('reports error when last_error is newer than last success', () => {
    const s = deriveStatus({ ...base, last_success_at: '2026-01-01T00:00:00Z', last_error: 'boom', last_error_at: '2026-01-02T00:00:00Z' });
    expect(s.kind).toBe('error');
  });
  it('reports matched', () => {
    expect(deriveStatus({ ...base, matched: true, last_success_at: '2026-01-02T00:00:00Z' }).kind).toBe('matched');
  });
  it('reports no_match', () => {
    expect(deriveStatus({ ...base, matched: false, last_success_at: '2026-01-02T00:00:00Z' }).kind).toBe('no_match');
  });
  it('reports unknown when never evaluated', () => {
    expect(deriveStatus(base).kind).toBe('unknown');
  });
  it('prefers success over a stale older error', () => {
    const s = deriveStatus({ ...base, matched: true, last_success_at: '2026-01-03T00:00:00Z', last_error: 'old', last_error_at: '2026-01-01T00:00:00Z' });
    expect(s.kind).toBe('matched');
  });
});
