import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import TargetList from '$lib/components/TargetList.svelte';
import { getTargets } from '$lib/api/client';
import { setToken } from '$lib/stores/token';

describe('targets API (MSW)', () => {
  beforeEach(() => setToken('test'));

  it('GET /targets returns the mocked targets', async () => {
    const data = await getTargets();
    expect(data).toHaveLength(2);
    expect(data[0].name).toBe('Campfire Mug');
  });

  it('renders the fetched targets in the list', async () => {
    const data = await getTargets();
    render(TargetList, { props: { targets: data, selectedId: undefined } });
    await waitFor(() => expect(screen.getByText('Campfire Mug')).toBeInTheDocument());
  });
});
