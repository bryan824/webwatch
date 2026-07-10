import { createSignal, Show } from 'solid-js';
import { Link } from '@tanstack/solid-router';
import { createNotifyMutation } from '../lib/mutations';
import { createOpsQuery } from '../lib/queries';
import { ConfirmDialog } from './ConfirmDialog';

interface Props {
  updatedLabel: string;
}

export function Topbar(props: Props) {
  const notify = createNotifyMutation();
  const ops = createOpsQuery();
  const [confirmOpen, setConfirmOpen] = createSignal(false);

  return (
    <header class="topbar">
      <div class="brand">
        <div class="brand__mark"><i /><u /></div>
        <span class="brand__name">web<b>watch</b></span>
      </div>
      <span class="topbar__stat mono">updated {props.updatedLabel}</span>
      <Show when={ops.data}>
        {(o) => (
          <span class="topbar__stat mono" title="scheduler / renderer / errors">
            {o().scheduler.running_targets} running · renderer {o().renderer_available ? 'ready' : o().renderer_configured ? 'down' : 'off'} · {o().targets.error} errors
          </span>
        )}
      </Show>
      <div class="topbar__spacer" />
      <Link to="/operations" class="btn sm">ops</Link>
      <Link to="/watches/new" class="btn sm primary">+ watch</Link>
      <a class="btn sm" href="/targets/export" download="targets.toml" title="Download all watches as targets.toml">export</a>
      <button class="btn sm" onClick={() => setConfirmOpen(true)}>send report</button>

      <ConfirmDialog
        open={confirmOpen()}
        onOpenChange={setConfirmOpen}
        title="Send Discord status report?"
        description="This re-checks every enabled target and posts one report to Discord. It can take a while."
        confirmLabel={notify.isPending ? 'sending...' : 'send report'}
        confirmDisabled={notify.isPending}
        onConfirm={() => notify.mutate(undefined)}
      />
    </header>
  );
}
