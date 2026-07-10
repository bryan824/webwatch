import { Show, For, createSignal } from 'solid-js';
import { createStore, produce } from 'solid-js/store';
import { Icon } from './Icon';
import { ConditionCard } from './ConditionCard';
import { ConfirmDialog } from './ConfirmDialog';
import {
  blankCondition,
  coerceForSubject,
  describeCondition,
  fromWire,
  validateAndBuild,
} from '../lib/conditions';
import type { Condition, Subject } from '../lib/conditions';
import { interpret } from '../lib/nl';
import {
  createAddTargetMutation,
  createDeleteTargetMutation,
  createDryRunMutation,
  createUpdateTargetMutation,
} from '../lib/mutations';
import type { DryRunResponse, RenderPlan, RenderPolicy, TargetInput, WatchConfig } from '../lib/types';

const INTERVALS = [
  { secs: 300, label: '5m' },
  { secs: 900, label: '15m' },
  { secs: 3600, label: '1h' },
  { secs: 21600, label: '6h' },
  { secs: 86400, label: 'daily' },
];

const RENDER_POLICIES: Array<{ value: RenderPolicy; label: string; help: string }> = [
  { value: 'auto', label: 'auto', help: 'Try HTTP first; render only when needed.' },
  { value: 'http_only', label: 'HTTP only', help: 'Fast and cheap, no browser.' },
  { value: 'render_first', label: 'render first', help: 'Use the browser immediately.' },
];

const NL_EXAMPLES = [
  'tell me when the mug is back in stock and under $25',
  'alert when "Sold out" is lifted',
  'when the price drops below $400',
];

interface Draft {
  name: string;
  url: string;
  enabled: boolean;
  intervalSecs: number;
  renderPolicy: RenderPolicy;
  mode: 'describe' | 'rules';
  nl: string;
  nlResult: { explanation: string; usedAi: boolean } | null;
  conditions: Condition[];
}

function defaultRenderPlan(policy: RenderPolicy = 'auto'): RenderPlan {
  return {
    policy,
    fingerprint_seed: null,
    wait_ms: null,
    scenario_match: 'any',
    steps: [],
    scenarios: [],
  };
}

function newDraft(): Draft {
  return {
    name: '',
    url: '',
    enabled: true,
    intervalSecs: 900,
    renderPolicy: 'auto',
    mode: 'describe',
    nl: '',
    nlResult: null,
    conditions: [],
  };
}

function draftFromTarget(t: WatchConfig): Draft {
  return {
    name: t.name,
    url: t.url,
    enabled: t.enabled,
    intervalSecs: t.interval_secs ?? 900,
    renderPolicy: t.render?.policy ?? 'auto',
    mode: 'rules',
    nl: '',
    nlResult: null,
    conditions: t.conditions.map(fromWire),
  };
}

interface Props {
  target?: WatchConfig;
  onSaved?: () => void;
  onCancel?: () => void;
  onDeleted?: () => void;
}

export function BuilderPane(props: Props) {
  const isEdit = () => !!props.target;
  const add = createAddTargetMutation();
  const update = createUpdateTargetMutation();
  const dryRun = createDryRunMutation();
  const del = createDeleteTargetMutation();
  const [error, setError] = createSignal('');
  const [confirmOpen, setConfirmOpen] = createSignal(false);
  const [justSaved, setJustSaved] = createSignal(false);
  const [dryRunResult, setDryRunResult] = createSignal<DryRunResponse | null>(null);

  const initial = props.target ? draftFromTarget(props.target) : newDraft();
  const [draft, setDraft] = createStore<Draft>(initial);

  function setUrl(url: string) { setDraft('url', url); }
  function setName(name: string) { setDraft('name', name); }
  function setNl(text: string) { setDraft('nl', text); }
  function setMode(mode: 'describe' | 'rules') { setDraft('mode', mode); }
  function setIntervalSecs(secs: number) { setDraft('intervalSecs', secs); }
  function setEnabled(on: boolean) { setDraft('enabled', on); }
  function setRenderPolicy(policy: RenderPolicy) { setDraft('renderPolicy', policy); }

  function runNl(text?: string) {
    const input = text ?? draft.nl;
    const result = interpret(input);
    if (!result.conditions.length) return;
    setDraft(produce((d) => {
      if (text != null) d.nl = text;
      d.conditions = result.conditions;
      d.nlResult = { explanation: result.explanation, usedAi: result.usedAi };
    }));
  }

  function addCondition() {
    setDraft(produce((d) => {
      d.conditions.push(blankCondition());
      d.mode = 'rules';
    }));
  }

  function changeSubject(cid: string, subjectId: Subject) {
    setDraft('conditions', (cs: Condition[]) =>
      cs.map((c) => c.cid === cid ? coerceForSubject({ ...c }, subjectId) : c)
    );
  }

  function updateCondition(cid: string, updated: Condition) {
    setDraft('conditions', (cs: Condition[]) =>
      cs.map((c) => c.cid === cid ? updated : c)
    );
  }

  function removeCondition(cid: string) {
    setDraft('conditions', (cs: Condition[]) => cs.filter((c) => c.cid !== cid));
  }

  function buildInput(): TargetInput | null {
    setError('');
    const n = draft.name.trim();
    if (!n) { setError('Name is required.'); return null; }

    const u = draft.url.trim();
    try { new URL(u); } catch { setError('Enter a valid absolute URL (https://...).'); return null; }

    if (draft.conditions.length === 0) { setError('Add at least one condition.'); return null; }

    const result = validateAndBuild(draft.conditions);
    if (!result.ok) { setError(result.error); return null; }

    return {
      name: n,
      url: u,
      enabled: draft.enabled,
      conditions: result.conditions,
      interval_secs: draft.intervalSecs,
      render: defaultRenderPlan(draft.renderPolicy),
    };
  }

  function runDryRun() {
    const input = buildInput();
    if (!input) return;
    dryRun.mutate({ ...input, target_id: props.target?.id }, {
      onSuccess: (result) => setDryRunResult(result),
    });
  }

  function save() {
    const input = buildInput();
    if (!input) return;
    const onSuccess = () => {
      setJustSaved(true);
      setTimeout(() => setJustSaved(false), 1600);
      props.onSaved?.();
    };

    if (props.target) {
      update.mutate({ id: props.target.id, input }, { onSuccess });
    } else {
      add.mutate(input, { onSuccess });
    }
  }

  function handleDelete() {
    if (!props.target) return;
    del.mutate(props.target.id, {
      onSuccess: () => props.onDeleted?.(),
    });
  }

  const summaryText = () => {
    if (!draft.conditions.length) return null;
    return draft.conditions.map(describeCondition).join('  ·  AND  ·  ');
  };

  const saving = () => add.isPending || update.isPending;

  return (
    <div class="builder-pane">
      <div>
        <div class="builder-pane__title">
          {isEdit() ? draft.name || 'Edit watch' : 'New watch'}
        </div>
        <div class="builder-pane__sub">
          Configure only supported rules, dry-run them, then save the watch.
        </div>
      </div>

      <div class="block">
        <div class="block__head">
          <span class="label">Target</span><span class="rule" />
        </div>
        <div class="block__body" style="display:grid;gap:14px">
          <div class="field">
            <span class="label">Page URL</span>
            <input
              class="input mono"
              placeholder="https://store.example.com/products/…"
              spellcheck={false}
              value={draft.url}
              onInput={(e) => setUrl(e.currentTarget.value)}
            />
          </div>
          <div class="field">
            <span class="label">Name</span>
            <input
              class="input"
              placeholder="My watch"
              value={draft.name}
              onInput={(e) => setName(e.currentTarget.value)}
            />
          </div>
          <div class="field">
            <span class="label">Render policy</span>
            <div class="presets" style="align-items:stretch">
              <For each={RENDER_POLICIES}>
                {(policy) => (
                  <button
                    class="preset"
                    data-on={(draft.renderPolicy === policy.value).toString()}
                    onClick={() => setRenderPolicy(policy.value)}
                    title={policy.help}
                  >
                    {policy.label}
                  </button>
                )}
              </For>
            </div>
          </div>
        </div>
      </div>

      <div class="block">
        <div class="block__head">
          <span class="label">What to watch</span>
          <span class="rule" />
          <div class="seg" role="group">
            <button data-on={(draft.mode === 'describe').toString()} onClick={() => setMode('describe')}>
              <Icon name="ai" /> Suggest rules
            </button>
            <button data-on={(draft.mode === 'rules').toString()} onClick={() => setMode('rules')}>
              <Icon name="edit" /> Build rules
            </button>
          </div>
        </div>
        <div class="block__body">
          <Show when={draft.mode === 'describe'}>
            <div>
              <div class="nl__row">
                <textarea
                  class="nl__input"
                  placeholder="Local heuristic only — e.g. the mug is back in stock and under $25"
                  spellcheck={false}
                  value={draft.nl}
                  onInput={(e) => setNl(e.currentTarget.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                      e.preventDefault();
                      runNl();
                    }
                  }}
                />
                <button class="btn ai" onClick={() => runNl()} title="⌘↵">
                  <Icon name="ai" /> Suggest
                </button>
              </div>
              <div class="nl__hint">
                <span style="color:var(--faint)">examples</span>
                <For each={NL_EXAMPLES}>
                  {(ex) => (
                    <button class="nl__chip" onClick={() => runNl(ex)}>
                      {ex}
                    </button>
                  )}
                </For>
              </div>
              <Show when={draft.nlResult}>
                {(nr) => (
                  <div class="understood">
                    <span class="understood__icon"><Icon name="ai" /></span>
                    <span class="understood__text">
                      {nr().explanation}{' '}
                      <span class="mono" style="color:var(--warn)">
                        · local setup helper, not production AI
                      </span>
                    </span>
                  </div>
                )}
              </Show>
            </div>
          </Show>

          <Show when={draft.mode === 'rules'}>
            <div>
              <For each={draft.conditions}>
                {(c, i) => (
                  <ConditionCard
                    index={i()}
                    condition={c}
                    onChange={(updated) => updateCondition(c.cid, updated)}
                    onChangeSubject={changeSubject}
                    onRemove={() => removeCondition(c.cid)}
                  />
                )}
              </For>
              <button class="add-cond" onClick={addCondition}>
                <Icon name="plus" /> Add a condition — alerts fire when ALL match
              </button>
            </div>
          </Show>
        </div>
      </div>

      <Show when={draft.conditions.length > 0}>
        <div class="block">
          <div class="summary">
            <span style="color:var(--accent);margin-top:1px"><Icon name="target" /></span>
            <span class="summary__txt">
              Alert me when <em>all</em> of: {summaryText()}
            </span>
          </div>
        </div>
      </Show>

      <div class="block">
        <div class="block__head">
          <span class="label">Evidence dry-run</span><span class="rule" />
          <button class="btn sm primary" onClick={runDryRun} disabled={dryRun.isPending || draft.conditions.length === 0}>
            {dryRun.isPending ? 'running...' : 'Run dry-run'}
          </button>
        </div>
        <div class="block__body">
          <Show
            when={dryRunResult()}
            fallback={<p style="color:var(--faint);font-size:12px;margin:0">Run before saving to prove extraction, matching, engine choice, and failure reason.</p>}
          >
            {(result) => (
              <div style="display:grid;gap:10px">
                <div class="meta-grid" style="margin-top:0">
                  <div class="meta-grid__cell"><div class="meta-grid__label">match</div><div class="meta-grid__value">{String(result().matched)}</div></div>
                  <div class="meta-grid__cell"><div class="meta-grid__label">engine</div><div class="meta-grid__value">{result().engine_used ?? '—'}</div></div>
                  <div class="meta-grid__cell"><div class="meta-grid__label">duration</div><div class="meta-grid__value">{result().duration_ms}ms</div></div>
                  <div class="meta-grid__cell"><div class="meta-grid__label">errors</div><div class="meta-grid__value">{result().diagnostics.length}</div></div>
                </div>
                <Show when={result().error}>
                  <div class="error-banner">{result().error}</div>
                </Show>
                <Show when={result().evidence.length > 0}>
                  <div class="evidence-box">
                    <For each={result().evidence}>{(e) => <p>{e}</p>}</For>
                  </div>
                </Show>
                <Show when={result().condition_results.length > 0}>
                  <div>
                    <div class="section__title">Condition evidence</div>
                    <For each={result().condition_results}>
                      {(c) => (
                        <div class="cond-result">
                          <span class="cond-result__kind">{c.condition_id}</span>
                          <span class={`cond-result__status ${c.matched ? 'pass' : 'fail'}`}>{c.matched ? 'pass' : 'fail'}</span>
                          <span class="cond-result__evidence truncate">{c.evidence.join(', ') || 'no evidence'}</span>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>
                <Show when={result().artifacts.html_url || result().artifacts.screenshot_url}>
                  <div style="display:flex;gap:8px">
                    <Show when={result().artifacts.html_url}>{(url) => <a class="btn sm" href={url()} target="_blank" rel="noreferrer">HTML ↗</a>}</Show>
                    <Show when={result().artifacts.screenshot_url}>{(url) => <a class="btn sm" href={url()} target="_blank" rel="noreferrer">screenshot ↗</a>}</Show>
                  </div>
                </Show>
              </div>
            )}
          </Show>
        </div>
      </div>

      <div class="block">
        <div class="block__head">
          <span class="label">Schedule</span><span class="rule" />
        </div>
        <div class="block__body sched">
          <span class="label" style="margin-right:2px">Check every</span>
          <div class="presets">
            <For each={INTERVALS}>
              {(iv) => (
                <button
                  class="preset"
                  data-on={(draft.intervalSecs === iv.secs).toString()}
                  onClick={() => setIntervalSecs(iv.secs)}
                >
                  {iv.label}
                </button>
              )}
            </For>
          </div>
          <span style="flex:1" />
          <label class="switch-row">
            <button
              class="ww-switch"
              data-checked={draft.enabled ? '' : undefined}
              onClick={() => setEnabled(!draft.enabled)}
              type="button"
            >
              <span class="ww-switch__thumb" />
            </button>
            <span>{draft.enabled ? 'enabled' : 'paused'}</span>
          </label>
        </div>
      </div>

      <Show when={error()}>
        <p style="color: var(--bad); font-size: 13px; margin: 0 0 12px">{error()}</p>
      </Show>

      <div class="savebar">
        <Show when={isEdit()}>
          <button class="as-trigger" onClick={() => setConfirmOpen(true)}>
            <Icon name="trash" /> Delete
          </button>
        </Show>
        <span class="savebar__spacer" />
        <button class="btn ghost" onClick={() => props.onCancel?.()}>Cancel</button>
        <button
          class="btn primary"
          onClick={save}
          disabled={saving() || draft.conditions.length === 0}
        >
          {justSaved()
            ? <><Icon name="check" /> Saved</>
            : saving()
              ? 'saving...'
              : isEdit() ? 'Save changes' : 'Create watch'}
        </button>
      </div>

      <ConfirmDialog
        open={confirmOpen()}
        onOpenChange={setConfirmOpen}
        title="Delete this watch?"
        description={`"${draft.name}" will stop being monitored. This can't be undone.`}
        confirmLabel="Delete watch"
        variant="danger"
        onConfirm={handleDelete}
      />
    </div>
  );
}
