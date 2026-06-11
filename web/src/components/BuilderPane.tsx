import { Show, For, createSignal } from 'solid-js';
import { createStore, produce } from 'solid-js/store';
import { Icon } from './Icon';
import { ConditionCard } from './ConditionCard';
import { ConfirmDialog } from './ConfirmDialog';
import {
  blankCondition, coerceForSubject, describeCondition,
  validateAndBuild,
} from '../lib/conditions';
import type { Condition, Subject } from '../lib/conditions';
import { interpret } from '../lib/nl';
import { createAddTargetMutation, createDeleteTargetMutation } from '../lib/mutations';
import type { TargetInput, TargetStatus } from '../lib/types';


const INTERVALS = [
  { secs: 300, label: '5m' },
  { secs: 900, label: '15m' },
  { secs: 3600, label: '1h' },
  { secs: 21600, label: '6h' },
  { secs: 86400, label: 'daily' },
];

const NL_EXAMPLES = [
  'tell me when the mug is back in stock and under $25',
  'alert when "Sold out" is lifted',
  'when the price drops below $400',
  'when the listing description changes',
];

interface Draft {
  name: string;
  url: string;
  enabled: boolean;
  intervalSecs: number;
  mode: 'describe' | 'rules';
  nl: string;
  nlResult: { explanation: string; usedAi: boolean } | null;
  conditions: Condition[];
}

function newDraft(): Draft {
  return {
    name: '', url: '', enabled: true, intervalSecs: 900,
    mode: 'describe', nl: '', nlResult: null, conditions: [],
  };
}

function draftFromTarget(t: TargetStatus): Draft {
  return {
    name: t.name,
    url: t.url,
    enabled: t.enabled,
    intervalSecs: 900,
    mode: 'rules',
    nl: '',
    nlResult: null,
    conditions: [blankCondition()],
  };
}

interface Props {
  target?: TargetStatus;
  onSaved?: () => void;
  onCancel?: () => void;
  onDeleted?: () => void;
}

export function BuilderPane(props: Props) {
  const isEdit = () => !!props.target;
  const add = createAddTargetMutation();
  const del = createDeleteTargetMutation();
  const [error, setError] = createSignal('');
  const [confirmOpen, setConfirmOpen] = createSignal(false);
  const [justSaved, setJustSaved] = createSignal(false);

  const initial = props.target ? draftFromTarget(props.target) : newDraft();
  const [draft, setDraft] = createStore<Draft>(initial);

  function setUrl(url: string) { setDraft('url', url); }
  function setName(name: string) { setDraft('name', name); }

  function setNl(text: string) { setDraft('nl', text); }
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

  function setMode(mode: 'describe' | 'rules') { setDraft('mode', mode); }

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

  function setIntervalSecs(secs: number) { setDraft('intervalSecs', secs); }
  function setEnabled(on: boolean) { setDraft('enabled', on); }

  function save() {
    setError('');
    const n = draft.name.trim();
    if (!n) { setError('Name is required.'); return; }

    const u = draft.url.trim();
    try { new URL(u); } catch { setError('Enter a valid absolute URL (https://...).'); return; }

    if (draft.conditions.length === 0) { setError('Add at least one condition.'); return; }

    const result = validateAndBuild(draft.conditions);
    if (!result.ok) { setError(result.error); return; }

    const input: TargetInput = {
      name: n,
      url: u,
      enabled: draft.enabled,
      conditions: result.conditions,
      interval_secs: draft.intervalSecs,
    };

    add.mutate(input, {
      onSuccess: () => {
        setJustSaved(true);
        setTimeout(() => setJustSaved(false), 1600);
        props.onSaved?.();
      },
    });
  }

  function handleDelete() {
    if (!props.target) return;
    del.mutate(props.target.target_id, {
      onSuccess: () => props.onDeleted?.(),
    });
  }

  const summaryText = () => {
    if (!draft.conditions.length) return null;
    return draft.conditions.map(describeCondition).join('  ·  AND  ·  ');
  };

  return (
    <div class="builder-pane">
      <div>
        <div class="builder-pane__title">
          {isEdit() ? draft.name || 'Edit watch' : 'New watch'}
        </div>
        <div class="builder-pane__sub">
          Point at a page, say what you want to know, and confirm what we'd alert on.
        </div>
      </div>

      {/* TARGET BLOCK */}
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
        </div>
      </div>

      {/* WHAT TO WATCH */}
      <div class="block">
        <div class="block__head">
          <span class="label">What to watch</span>
          <span class="rule" />
          <div class="seg" role="group">
            <button data-on={(draft.mode === 'describe').toString()} onClick={() => setMode('describe')}>
              <Icon name="ai" /> Describe
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
                  placeholder="Tell me when… (e.g. the mug is back in stock and under $25)"
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
                <button class="btn ai" onClick={runNl} title="⌘↵">
                  <Icon name="ai" /> Understand
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
                      {nr().explanation}
                      <Show when={nr().usedAi}>
                        {' '}<span class="mono" style="color:var(--ai)">
                          · AI resolved this at setup; checks stay deterministic.
                        </span>
                      </Show>
                    </span>
                  </div>
                )}
              </Show>
              <Show when={draft.conditions.length > 0}>
                <div style="margin-top:14px;display:grid;gap:8px">
                  <For each={draft.conditions}>
                    {(c, i) => (
                      <div class="cond" style="background:var(--surface)">
                        <div class="cond__top" style="padding:11px 12px">
                          <span class="cond__idx" style="padding-top:2px">
                            {String(i() + 1).padStart(2, '0')}
                          </span>
                          <div class="cond__grid" style="align-items:center">
                            <span style="font-size:13px">{describeCondition(c)}</span>
                          </div>
                          <button class="btn ghost sm" onClick={() => setMode('rules')}>
                            <Icon name="edit" /> edit as rule
                          </button>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
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

      {/* SUMMARY */}
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

      {/* SCHEDULE */}
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

      {/* ERROR */}
      <Show when={error()}>
        <p style="color: var(--bad); font-size: 13px; margin: 0 0 12px">{error()}</p>
      </Show>

      {/* SAVE BAR */}
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
          disabled={add.isPending || draft.conditions.length === 0}
        >
          {justSaved()
            ? <><Icon name="check" /> Saved</>
            : add.isPending
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
