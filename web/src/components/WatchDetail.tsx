import { Show, For } from 'solid-js';
import { Link } from '@tanstack/solid-router';
import { deriveStatus } from '../lib/status';
import { formatPrice, formatRelative } from '../lib/format';
import { StatusBadge } from './StatusBadge';
import { StatusDot } from './StatusDot';
import type { CheckRun, TargetStatus, ConditionResult } from '../lib/types';

interface Props {
  target: TargetStatus;
  checking: boolean;
  mutating: boolean;
  onCheckNow: () => void;
  onToggleEnabled: () => void;
  onDelete: () => void;
  checks: CheckRun[];
}

function CondResultRow(props: { c: ConditionResult }) {
  return (
    <div class="cond-result">
      <span class="cond-result__kind">{props.c.kind}</span>
      <span class={`cond-result__status ${props.c.matched ? 'pass' : 'fail'}`}>
        {props.c.matched ? 'pass' : 'fail'}
      </span>
      <Show when={props.c.evidence.length > 0}>
        <span class="cond-result__evidence truncate">{props.c.evidence.join(', ')}</span>
      </Show>
      <Show when={props.c.error}>
        <span class="cond-result__error">{props.c.error}</span>
      </Show>
    </div>
  );
}

export function WatchDetail(props: Props) {
  const s = () => deriveStatus(props.target);

  const meta = () => [
    ['engine', props.target.engine_used ?? '—'],
    ['price', formatPrice(props.target.price_cents)],
    ['render', props.target.render?.policy ?? 'auto'],
    ['last success', formatRelative(props.target.last_success_at)],
    ['last alert', formatRelative(props.target.last_alert_at)],
  ] as const;

  return (
    <div class="detail">
      <div class="detail__header">
        <div style="min-width: 0">
          <div style="display: flex; align-items: center; gap: 8px">
            <h1 class="detail__title truncate">{props.target.name}</h1>
            <Show when={!props.target.enabled}>
              <span class="detail__disabled-badge">disabled</span>
            </Show>
          </div>
          <a
            class="detail__url truncate"
            href={props.target.url}
            target="_blank"
            rel="noreferrer"
          >
            {props.target.url} ↗
          </a>
        </div>
        <div class="detail__actions">
          <Link to="/watches/$id/edit" params={{ id: props.target.target_id }} class="btn sm">
            edit
          </Link>
          <button class="btn sm" onClick={props.onToggleEnabled} disabled={props.mutating}>
            {props.target.enabled ? 'disable' : 'enable'}
          </button>
          <button class="btn sm primary" onClick={props.onCheckNow} disabled={props.checking}>
            {props.checking ? 'checking...' : 'check now'}
          </button>
          <button class="btn sm danger" onClick={props.onDelete} disabled={props.mutating}>
            delete
          </button>
        </div>
      </div>

      <StatusBadge target={props.target} />

      <Show when={s().kind === 'error' && props.target.last_error}>
        <div class="error-banner">
          {props.target.last_error}
          <span class="ts"> · {formatRelative(props.target.last_error_at)}</span>
        </div>
      </Show>

      <Show when={s().kind === 'unknown' && props.target.condition_results.length === 0}>
        <p style="margin-top: 20px; color: var(--muted); font-size: 13px">
          Not checked yet — click <span style="color: var(--accent)">check now</span> to evaluate.
        </p>
      </Show>

      <Show when={s().kind !== 'unknown' || props.target.condition_results.length > 0}>
        <div class="meta-grid">
          <For each={meta()}>
            {([label, value]) => (
              <div class="meta-grid__cell">
                <div class="meta-grid__label">{label}</div>
                <div class="meta-grid__value">{value}</div>
              </div>
            )}
          </For>
        </div>

        <div class="section">
          <div class="section__title">Evidence</div>
          <Show
            when={props.target.evidence.length > 0}
            fallback={<p style="color: var(--faint); font-size: 12px">no evidence</p>}
          >
            <div class="evidence-box">
              <For each={props.target.evidence}>
                {(e) => <p>{e}</p>}
              </For>
            </div>
          </Show>
        </div>

        <div class="section">
          <div class="section__title">Conditions</div>
          <Show
            when={props.target.condition_results.length > 0}
            fallback={<p style="color: var(--faint); font-size: 12px">no condition results</p>}
          >
            <For each={props.target.condition_results}>
              {(c) => <CondResultRow c={c} />}
            </For>
          </Show>
        </div>
      </Show>

      <div class="section">
        <div class="section__title">Recent runs</div>
        <Show
          when={props.checks.length > 0}
          fallback={<p style="color: var(--faint); font-size: 12px">no runs recorded yet</p>}
        >
          <For each={props.checks}>
            {(run) => (
              <div class="cond-result">
                <span class="cond-result__kind">{formatRelative(run.checked_at)}</span>
                <span class={`cond-result__status ${run.error ? 'fail' : run.matched ? 'pass' : 'fail'}`}>
                  {run.error ? 'error' : run.matched ? 'match' : 'no match'}
                </span>
                <span class="cond-result__evidence truncate">
                  {run.error || `${run.engine_used ?? '—'} · ${run.evidence.join(', ') || 'no evidence'}`}
                </span>
              </div>
            )}
          </For>
        </Show>
      </div>

      <Show when={props.target.engine_used === 'browser_cdp' || !!props.target.last_error}>
        <div class="section">
          <div class="section__title">Last render</div>
          <div style="display: flex; gap: 8px">
            <a
              class="btn sm"
              href={`/targets/${encodeURIComponent(props.target.target_id)}/snapshot.html`}
              target="_blank"
              rel="noreferrer"
            >
              view HTML ↗
            </a>
            <a
              class="btn sm"
              href={`/targets/${encodeURIComponent(props.target.target_id)}/snapshot.png`}
              target="_blank"
              rel="noreferrer"
            >
              screenshot ↗
            </a>
          </div>
          <p style="color: var(--faint); font-size: 12px; margin-top: 6px">
            What the browser engine actually captured on the last check.
          </p>
        </div>
      </Show>
    </div>
  );
}
