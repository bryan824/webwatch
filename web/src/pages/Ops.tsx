import { For, Show } from 'solid-js';
import { Link } from '@tanstack/solid-router';
import { createOpsQuery } from '../lib/queries';
import { createTestDiscordMutation, createTestRendererMutation } from '../lib/mutations';
import { formatRelative } from '../lib/format';
import type { IntegrationTestResponse } from '../lib/types';

function IntegrationResult(props: { result?: IntegrationTestResponse }) {
  return (
    <Show when={props.result}>
      {(result) => (
        <div class="evidence-box" style="margin-top:10px">
          <p>{result().message}</p>
          <For each={result().checks}>
            {(check) => (
              <div class="cond-result">
                <span class="cond-result__kind">{check.name}</span>
                <span class={`cond-result__status ${check.ok ? 'pass' : 'fail'}`}>
                  {check.ok ? 'ok' : 'fail'}
                </span>
                <span class="cond-result__evidence truncate">{check.message}</span>
              </div>
            )}
          </For>
        </div>
      )}
    </Show>
  );
}

export function Ops() {
  const ops = createOpsQuery();
  const discordTest = createTestDiscordMutation();
  const rendererTest = createTestRendererMutation();

  return (
    <div class="detail">
      <div class="detail__header">
        <div>
          <h1 class="detail__title">Operations</h1>
          <p style="color:var(--faint);font-size:13px;margin:4px 0 0">
            Runtime health, scheduler state, renderer readiness, and recent failures.
          </p>
        </div>
      </div>

      <Show
        when={ops.data}
        fallback={<p style="color: var(--faint); font-size: 13px">Loading operations status...</p>}
      >
        {(o) => (
          <>
            <div class="meta-grid">
              <div class="meta-grid__cell"><div class="meta-grid__label">API</div><div class="meta-grid__value">{o().status}</div></div>
              <div class="meta-grid__cell"><div class="meta-grid__label">DB</div><div class="meta-grid__value">{o().persistence_backend}</div></div>
              <div class="meta-grid__cell"><div class="meta-grid__label">scheduler</div><div class="meta-grid__value">{o().scheduler.running_targets} running</div></div>
              <div class="meta-grid__cell"><div class="meta-grid__label">discord</div><div class="meta-grid__value">{o().discord_configured ? 'configured' : 'off'}</div></div>
              <div class="meta-grid__cell"><div class="meta-grid__label">renderer</div><div class="meta-grid__value">{o().renderer_available ? 'ready' : o().renderer_configured ? 'configured/down' : 'off'}</div></div>
            </div>

            <div class="section">
              <div class="section__title">Integrations</div>
              <div class="meta-grid" style="margin-top:0; grid-template-columns: repeat(2, 1fr)">
                <div class="meta-grid__cell">
                  <div class="row" style="justify-content:space-between; align-items:flex-start">
                    <div>
                      <div class="meta-grid__label">Discord webhook</div>
                      <div class="meta-grid__value">{o().discord_configured ? 'configured' : 'not configured'}</div>
                    </div>
                    <button class="btn sm" disabled={discordTest.isPending} onClick={() => discordTest.mutate(undefined)}>
                      {discordTest.isPending ? 'testing...' : 'send test'}
                    </button>
                  </div>
                  <IntegrationResult result={discordTest.data} />
                </div>
                <div class="meta-grid__cell">
                  <div class="row" style="justify-content:space-between; align-items:flex-start">
                    <div>
                      <div class="meta-grid__label">CDP renderer</div>
                      <div class="meta-grid__value">{o().renderer_available ? 'ready' : o().renderer_configured ? 'configured/down' : 'off'}</div>
                    </div>
                    <button class="btn sm" disabled={rendererTest.isPending} onClick={() => rendererTest.mutate(undefined)}>
                      {rendererTest.isPending ? 'testing...' : 'test CDP'}
                    </button>
                  </div>
                  <IntegrationResult result={rendererTest.data} />
                </div>
              </div>
            </div>

            <div class="section">
              <div class="section__title">Watch counts</div>
              <div class="meta-grid" style="margin-top:0">
                <div class="meta-grid__cell"><div class="meta-grid__label">total</div><div class="meta-grid__value">{o().targets.total}</div></div>
                <div class="meta-grid__cell"><div class="meta-grid__label">enabled</div><div class="meta-grid__value">{o().targets.enabled}</div></div>
                <div class="meta-grid__cell"><div class="meta-grid__label">matched</div><div class="meta-grid__value">{o().targets.matched}</div></div>
                <div class="meta-grid__cell"><div class="meta-grid__label">errors</div><div class="meta-grid__value">{o().targets.error}</div></div>
              </div>
            </div>

            <div class="section">
              <div class="section__title">Recent errors</div>
              <Show
                when={o().recent_errors.length > 0}
                fallback={<p style="color: var(--faint); font-size: 12px">no target errors</p>}
              >
                <For each={o().recent_errors}>
                  {(err) => (
                    <div class="cond-result">
                      <span class="cond-result__kind"><Link to="/watches/$id" params={{ id: err.target_id }}>{err.name}</Link></span>
                      <span class="cond-result__status fail">error</span>
                      <span class="cond-result__evidence truncate">{err.error} · {formatRelative(err.at)}</span>
                    </div>
                  )}
                </For>
              </Show>
            </div>
          </>
        )}
      </Show>
    </div>
  );
}
