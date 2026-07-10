// vendor.js — single place that pulls Solid + Base UI from the import map / CDN.
// Keeping every import here means the esm.sh/version details live in one file and
// every other module imports from "./vendor.js".

export { render } from 'solid-js/web';
export {
  createSignal,
  createMemo,
  createEffect,
  For,
  Show,
  onMount,
  onCleanup,
  batch,
} from 'solid-js';
export { createStore, produce } from 'solid-js/store';
export { default as html } from 'solid-js/html';

// Base UI (Solid port). Root barrels each component as a namespace.
export { Switch, Tabs, Tooltip, AlertDialog } from '@msviderok/base-ui-solid';
