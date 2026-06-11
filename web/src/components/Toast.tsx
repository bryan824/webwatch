import { createSignal, For, onCleanup } from 'solid-js';

export type ToastKind = 'success' | 'error' | 'info';

interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

const [toasts, setToasts] = createSignal<Toast[]>([]);
let nextId = 0;

export function addToast(message: string, kind: ToastKind = 'info') {
  const id = nextId++;
  setToasts((prev) => [...prev, { id, message, kind }]);
  setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 3500);
}

export function ToastContainer() {
  return (
    <div class="toast-container">
      <For each={toasts()}>
        {(t) => <div class={`toast ${t.kind}`}>{t.message}</div>}
      </For>
    </div>
  );
}
