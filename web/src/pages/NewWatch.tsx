import { Show, createSignal } from 'solid-js';
import { useNavigate } from '@tanstack/solid-router';
import { BuilderPane } from '../components/BuilderPane';
import { Icon } from '../components/Icon';
import { createImportTargetsMutation } from '../lib/mutations';

type Mode = 'builder' | 'paste';

const TOML_PLACEHOLDER = `[[targets]]
id = "apple-refurb-mac-mini"
name = "Apple Refurbished Mac mini back in stock"
url = "https://www.apple.com/shop/refurbished/mac/mac-mini"
enabled = true

[[targets.conditions]]
id = "mac-mini-listed"
kind = "url_unchanged"`;

export function NewWatch() {
  const navigate = useNavigate();
  const [mode, setMode] = createSignal<Mode>('builder');
  const [toml, setToml] = createSignal('');
  const importTargets = createImportTargetsMutation();

  function runImport() {
    const body = toml().trim();
    if (!body) return;
    importTargets.mutate(body, { onSuccess: () => navigate({ to: '/' }) });
  }

  return (
    <>
      <div class="mode-bar">
        <div class="seg" role="group" aria-label="New watch mode">
          <button data-on={(mode() === 'builder').toString()} onClick={() => setMode('builder')}>
            <Icon name="edit" /> Builder
          </button>
          <button data-on={(mode() === 'paste').toString()} onClick={() => setMode('paste')}>
            <Icon name="target" /> Paste config
          </button>
        </div>
      </div>

      <Show
        when={mode() === 'builder'}
        fallback={
          <div class="builder-pane">
            <div>
              <div class="builder-pane__title">Paste config</div>
              <div class="builder-pane__sub">
                Paste one or more <span class="mono">[[targets]]</span> blocks in{' '}
                <span class="mono">targets.toml</span> format. Each block needs an{' '}
                <span class="mono">id</span> — an existing id updates that watch in place.
              </div>
            </div>

            <div class="block">
              <div class="block__head">
                <span class="label">targets.toml</span><span class="rule" />
              </div>
              <div class="block__body">
                <textarea
                  class="input mono"
                  style="width:100%;min-height:280px;resize:vertical;line-height:1.5"
                  spellcheck={false}
                  placeholder={TOML_PLACEHOLDER}
                  value={toml()}
                  onInput={(e) => setToml(e.currentTarget.value)}
                />
                <Show when={importTargets.isError}>
                  <div class="error-banner" style="margin-top:10px">
                    {(importTargets.error as Error)?.message ?? 'Import failed'}
                  </div>
                </Show>
              </div>
            </div>

            <div class="savebar">
              <span class="savebar__spacer" />
              <button class="btn ghost" onClick={() => navigate({ to: '/' })}>Cancel</button>
              <button
                class="btn primary"
                onClick={runImport}
                disabled={importTargets.isPending || !toml().trim()}
              >
                {importTargets.isPending ? 'importing...' : <><Icon name="plus" /> Import watches</>}
              </button>
            </div>
          </div>
        }
      >
        <BuilderPane onSaved={() => navigate({ to: '/' })} onCancel={() => navigate({ to: '/' })} />
      </Show>
    </>
  );
}
