// web/src/lib/components/TokenDialog.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';
import TokenDialog from './TokenDialog.svelte';
import { token, clearToken } from '$lib/stores/token';

describe('TokenDialog', () => {
  beforeEach(() => { localStorage.clear(); clearToken(); });

  it('saves the entered token to the store', async () => {
    render(TokenDialog, { props: { open: true } });
    const input = screen.getByLabelText(/api token/i) as HTMLInputElement;
    // Use fireEvent.input to set the value and trigger Svelte 5 bind:value reactivity
    fireEvent.input(input, { target: { value: 'my-secret' } });
    await userEvent.click(screen.getByRole('button', { name: /save/i }));
    expect(get(token)).toBe('my-secret');
  });
});
