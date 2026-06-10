import { Show, For } from 'solid-js';
import { Icon } from './Icon';
import type { Condition, Subject } from '../lib/conditions';
import { SUBJECTS, opsFor, valueField, locatorMode } from '../lib/conditions';

interface Props {
  index: number;
  condition: Condition;
  onChange: (c: Condition) => void;
  onChangeSubject: (cid: string, subject: Subject) => void;
  onRemove: () => void;
}

export function ConditionCard(props: Props) {
  const c = () => props.condition;
  const opOpts = () => opsFor(c().subject);
  const vf = () => valueField(c().subject, c().op);
  const locMode = () => locatorMode(c().subject);

  function patch(updates: Partial<Condition>) {
    props.onChange({ ...c(), ...updates });
  }

  return (
    <div class="cond">
      <div class="cond__top">
        <span class="cond__idx">{String(props.index + 1).padStart(2, '0')}</span>
        <div class="cond__grid">
          <select
            class="nselect"
            value={c().subject}
            onChange={(e) => props.onChangeSubject(c().cid, e.currentTarget.value as Subject)}
          >
            <For each={SUBJECTS}>
              {(s) => <option value={s.id}>{s.label}</option>}
            </For>
          </select>

          <select
            class="nselect"
            value={c().op}
            onChange={(e) => patch({ op: e.currentTarget.value })}
          >
            <For each={opOpts()}>
              {(o) => <option value={o.id}>{o.label}</option>}
            </For>
          </select>

          <Show when={vf()}>
            {(field) => (
              field().kind === 'money' ? (
                <span style="display:inline-flex;align-items:center;gap:0;background:var(--bg);border:1px solid var(--line-2);border-radius:var(--r)">
                  <span class="mono" style="padding-left:10px;color:var(--faint)">$</span>
                  <input
                    class="input mono"
                    style="width:92px;border:0;background:transparent"
                    inputmode="decimal"
                    placeholder={field().placeholder}
                    value={c().value}
                    onInput={(e) => patch({ value: e.currentTarget.value })}
                  />
                </span>
              ) : (
                <input
                  class="input"
                  style="width:200px;flex:1;min-width:120px"
                  placeholder={field().placeholder}
                  value={c().value}
                  onInput={(e) => patch({ value: e.currentTarget.value })}
                />
              )
            )}
          </Show>

          <div class="seg truth" role="group" aria-label="Alert when">
            <button
              data-on={(!c().negate).toString()}
              data-truth="true"
              onClick={() => patch({ negate: false })}
            >
              TRUE
            </button>
            <button
              data-on={c().negate.toString()}
              data-truth="false"
              onClick={() => patch({ negate: true })}
            >
              FALSE
            </button>
          </div>
        </div>
        <button class="cond__remove" aria-label="Remove condition" onClick={props.onRemove}>
          <Icon name="trash" />
        </button>
      </div>

      <Show when={locMode() !== 'page'}>
        <LocatorRow condition={c()} mode={locMode() as 'required' | 'optional'} onPatch={patch} />
      </Show>
    </div>
  );
}

function LocatorRow(props: {
  condition: Condition;
  mode: 'required' | 'optional';
  onPatch: (updates: Partial<Condition>) => void;
}) {
  const c = () => props.condition;
  const types = () => props.mode === 'required' ? ['css', 'xpath'] as const : ['page', 'css', 'xpath'] as const;
  const tlabel: Record<string, string> = { page: 'whole page', css: 'CSS', xpath: 'XPath' };

  return (
    <div class="cond__advanced">
      <div class="adv-grid">
        <span class="label">Where to look{props.mode === 'optional' ? ' · optional' : ''}</span>
        <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;min-width:0">
          <div class="seg" role="group" aria-label="Locator type">
            <For each={[...types()]}>
              {(t) => (
                <button
                  data-on={(c().locator.type === t).toString()}
                  onClick={() => props.onPatch({
                    locator: { type: t, query: t === 'page' ? '' : c().locator.query },
                  })}
                >
                  {tlabel[t]}
                </button>
              )}
            </For>
          </div>
          <Show when={c().locator.type !== 'page'}>
            <input
              class="input mono"
              style="flex:1;min-width:160px"
              spellcheck={false}
              placeholder={c().locator.type === 'xpath' ? "//span[@class='price']" : '.price'}
              value={c().locator.query}
              onInput={(e) => props.onPatch({
                locator: { ...c().locator, query: e.currentTarget.value },
              })}
            />
          </Show>
        </div>
        <span class="label">How</span>
        <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap">
          <div class="seg" role="group" aria-label="Extraction strategy">
            <For each={['auto', 'exact', 'keyword', 'ai'] as const}>
              {(s) => (
                <button
                  data-on={(c().strategy === s).toString()}
                  onClick={() => props.onPatch({ strategy: s })}
                >
                  {s}
                </button>
              )}
            </For>
          </div>
        </div>
      </div>
    </div>
  );
}
