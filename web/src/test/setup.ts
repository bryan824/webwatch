// web/src/test/setup.ts
import '@testing-library/jest-dom/vitest';
import { afterAll, afterEach, beforeAll } from 'vitest';
import { setupServer } from 'msw/node';
import { handlers } from './msw-handlers';

// Node 26 ships an experimental localStorage that blocks vitest's jsdom
// environment from copying jsdom's own localStorage to globalThis (the filter
// in populateGlobal skips keys that already exist on global). We patch
// globalThis here using the underlying jsdom window so that localStorage /
// sessionStorage are the jsdom-backed implementations that behave correctly in
// tests.
const _jsdomWin = (globalThis as Record<string, unknown>).jsdom as
  | { window: { localStorage: Storage; sessionStorage: Storage } }
  | undefined;
if (_jsdomWin?.window) {
  Object.defineProperty(globalThis, 'localStorage', {
    value: _jsdomWin.window.localStorage,
    writable: true,
    configurable: true,
  });
  Object.defineProperty(globalThis, 'sessionStorage', {
    value: _jsdomWin.window.sessionStorage,
    writable: true,
    configurable: true,
  });
}

export const server = setupServer(...handlers);
beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());
