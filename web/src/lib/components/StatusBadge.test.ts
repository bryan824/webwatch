// web/src/lib/components/StatusBadge.test.ts
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import StatusBadge from './StatusBadge.svelte';
import type { TargetStatus } from '$lib/api/types';

const t = (over: Partial<TargetStatus>): TargetStatus => ({
  target_id: 'x', name: 'X', url: 'https://e.com', matched: null, engine_used: null,
  price_cents: null, evidence: [], condition_results: [], last_success_at: null,
  last_error_at: null, last_error: null, last_alert_at: null, ...over
});

describe('StatusBadge', () => {
  it('shows Matched', () => {
    render(StatusBadge, { props: { target: t({ matched: true, last_success_at: '2026-01-02T00:00:00Z' }) } });
    expect(screen.getByText('Matched')).toBeInTheDocument();
  });
  it('shows Error', () => {
    render(StatusBadge, { props: { target: t({ last_error: 'boom', last_error_at: '2026-01-02T00:00:00Z' }) } });
    expect(screen.getByText('Error')).toBeInTheDocument();
  });
});
