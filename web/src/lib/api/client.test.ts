// web/src/lib/api/client.test.ts
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { apiFetch, ApiError } from './client';
import { setToken, clearToken } from '../stores/token';

describe('apiFetch', () => {
  beforeEach(() => { clearToken(); vi.restoreAllMocks(); });
  afterEach(() => vi.restoreAllMocks());

  it('attaches the bearer token when present', async () => {
    setToken('abc');
    const spy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), { status: 200, headers: { 'content-type': 'application/json' } })
    );
    await apiFetch('/targets');
    const init = spy.mock.calls[0][1] as RequestInit;
    expect(new Headers(init.headers).get('authorization')).toBe('Bearer abc');
  });

  it('throws ApiError with the server error message on non-2xx', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ error: 'boom' }), { status: 500, headers: { 'content-type': 'application/json' } })
    );
    await expect(apiFetch('/targets')).rejects.toMatchObject({ status: 500, message: 'boom' });
  });

  it('marks 401 errors as unauthorized', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ error: 'nope' }), { status: 401, headers: { 'content-type': 'application/json' } })
    );
    const err = await apiFetch('/targets').catch((e) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(401);
  });
});
