import { Show, For } from 'solid-js';
import { Outlet, useParams } from '@tanstack/solid-router';
import { Topbar } from './Topbar';
import { WatchList } from './WatchList';
import { createTargetsQuery } from '../lib/queries';
import { formatRelative } from '../lib/format';

export function Shell() {
  const targets = createTargetsQuery();
  const params = useParams({ from: '/watches/$id', shouldThrow: false });

  const updatedLabel = () =>
    formatRelative(
      targets.dataUpdatedAt ? new Date(targets.dataUpdatedAt).toISOString() : null
    );

  return (
    <div class="app">
      <Topbar updatedLabel={updatedLabel()} />
      <div class="main">
        <Show
          when={!targets.isPending}
          fallback={
            <div class="rail" style="padding: 12px">
              <For each={Array.from({ length: 5 })}>
                {() => <div class="skeleton" style="margin-bottom: 6px" />}
              </For>
            </div>
          }
        >
          <Show when={targets.error && !(targets.data)}>
            <div class="rail" style="padding: 16px">
              <p class="mono" style="font-size: 12px; color: var(--bad); word-break: break-word">
                {(targets.error as Error).message}
              </p>
              <button class="btn sm" style="margin-top: 8px" onClick={() => targets.refetch()}>
                retry
              </button>
            </div>
          </Show>
          <Show when={!targets.error || targets.data}>
            <Show
              when={(targets.data ?? []).length > 0}
              fallback={
                <div class="rail" style="padding: 16px">
                  <p style="color: var(--faint); font-size: 12px">
                    No watches yet — click <span style="color: var(--accent)">+ watch</span> to add one.
                  </p>
                </div>
              }
            >
              <WatchList targets={targets.data ?? []} selectedId={params()?.id} />
            </Show>
          </Show>
        </Show>

        <div class="pane">
          <Outlet />
        </div>
      </div>
    </div>
  );
}
