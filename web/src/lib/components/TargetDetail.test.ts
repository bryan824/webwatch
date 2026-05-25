// web/src/lib/components/TargetDetail.test.ts
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import TargetDetail from './TargetDetail.svelte';
import { sampleTargets } from '$test/msw-handlers';

describe('TargetDetail', () => {
  it('shows evidence and condition results', () => {
    render(TargetDetail, { props: { target: sampleTargets[0], checking: false, onCheckNow: () => {} } });
    // The evidence string appears in both the Evidence section and inside ConditionResultRow
    expect(screen.getAllByText('"Add to cart" found')[0]).toBeInTheDocument();
    expect(screen.getByText(/text/)).toBeInTheDocument();
  });

  it('fires onCheckNow when the button is clicked', async () => {
    const onCheckNow = vi.fn();
    render(TargetDetail, { props: { target: sampleTargets[0], checking: false, onCheckNow } });
    await userEvent.click(screen.getByRole('button', { name: /check now/i }));
    expect(onCheckNow).toHaveBeenCalledOnce();
  });

  it('shows an unknown empty-state for never-checked targets', () => {
    render(TargetDetail, { props: { target: sampleTargets[1], checking: false, onCheckNow: () => {} } });
    expect(screen.getByText(/not checked yet/i)).toBeInTheDocument();
  });
});
