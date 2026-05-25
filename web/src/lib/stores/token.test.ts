// web/src/lib/stores/token.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { token, hasToken, setToken, clearToken } from './token';

describe('token store', () => {
  beforeEach(() => {
    localStorage.clear();
    clearToken();
  });

  it('starts empty', () => {
    expect(get(token)).toBeNull();
    expect(get(hasToken)).toBe(false);
  });

  it('persists to localStorage on set', () => {
    setToken('secret');
    expect(get(token)).toBe('secret');
    expect(get(hasToken)).toBe(true);
    expect(localStorage.getItem('webwatch_token')).toBe('secret');
  });

  it('clears the token and storage', () => {
    setToken('secret');
    clearToken();
    expect(get(token)).toBeNull();
    expect(localStorage.getItem('webwatch_token')).toBeNull();
  });
});
