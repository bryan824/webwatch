// web/src/lib/components/TargetList.test.ts
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import TargetList from './TargetList.svelte';
import { sampleTargets } from '$test/msw-handlers';

describe('TargetList', () => {
  it('renders all targets and a summary count', () => {
    render(TargetList, { props: { targets: sampleTargets, selectedId: undefined } });
    expect(screen.getByText('Campfire Mug')).toBeInTheDocument();
    expect(screen.getByText('Sale Price Watch')).toBeInTheDocument();
    expect(screen.getByText(/2 targets/i)).toBeInTheDocument();
  });

  it('filters by search text', async () => {
    render(TargetList, { props: { targets: sampleTargets, selectedId: undefined } });
    await userEvent.type(screen.getByPlaceholderText(/search/i), 'campfire');
    expect(screen.getByText('Campfire Mug')).toBeInTheDocument();
    expect(screen.queryByText('Sale Price Watch')).not.toBeInTheDocument();
  });
});
