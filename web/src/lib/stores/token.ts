// web/src/lib/stores/token.ts
import { writable, derived } from 'svelte/store';

const KEY = 'webwatch_token';
const hasLS = typeof localStorage !== 'undefined';
const initial = hasLS ? localStorage.getItem(KEY) : null;

export const token = writable<string | null>(initial);
export const hasToken = derived(token, ($t) => !!$t && $t.length > 0);

export function setToken(value: string): void {
  const v = value.trim();
  if (hasLS) localStorage.setItem(KEY, v);
  token.set(v);
}

export function clearToken(): void {
  if (hasLS) localStorage.removeItem(KEY);
  token.set(null);
}
