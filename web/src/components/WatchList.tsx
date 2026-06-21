import { createSignal, For, Show, createMemo } from 'solid-js';
import { deriveStatus } from '../lib/status';
import { WatchListItem } from './WatchListItem';
import type { TargetStatus } from '../lib/types';

interface Props {
  targets: TargetStatus[];
  selectedId?: string;
}

export function WatchList(props: Props) {
  const [query, setQuery] = createSignal('');

  const filtered = createMemo(() => {
    const q = query().toLowerCase();
    if (!q) return props.targets;
    return props.targets.filter((t) => `${t.name} ${t.url}`.toLowerCase().includes(q));
  });

  const matched = createMemo(() =>
    props.targets.filter((t) => deriveStatus(t).kind === 'matched').length
  );
  const errored = createMemo(() =>
    props.targets.filter((t) => deriveStatus(t).kind === 'error').length
  );

  return (
    <div class="rail">
      <div class="rail__head">
        <div style="display: flex; flex-direction: column; gap: 8px; width: 100%">
          <input
            class="rail__search"
            placeholder="search watches..."
            value={query()}
            onInput={(e) => setQuery(e.currentTarget.value)}
          />
          <div class="rail__stats">
            <span>{props.targets.length} watches</span>
            <span style="color: var(--ok)">{matched()} matched</span>
            <Show when={errored() > 0}>
              <span style="color: var(--bad)">{errored()} err</span>
            </Show>
          </div>
        </div>
      </div>
      <div class="rail__list">
        <For each={filtered()} fallback={<p style="color: var(--faint); font-size: 12px; text-align: center; padding: 24px 0">no matching watches</p>}>
          {(t) => <WatchListItem target={t} selected={t.target_id === props.selectedId} />}
        </For>
      </div>
    </div>
  );
}
