import { createSignal } from 'solid-js';
import { Link } from '@tanstack/solid-router';
import { createReloadMutation, createNotifyMutation } from '../lib/mutations';
import { ConfirmDialog } from './ConfirmDialog';

interface Props {
  updatedLabel: string;
}

export function Topbar(props: Props) {
  const reload = createReloadMutation();
  const notify = createNotifyMutation();
  const [confirmOpen, setConfirmOpen] = createSignal(false);

  return (
    <header class="topbar">
      <div class="brand">
        <div class="brand__mark"><i /><u /></div>
        <span class="brand__name">web<b>watch</b></span>
      </div>
      <span class="topbar__stat mono">updated {props.updatedLabel}</span>
      <div class="topbar__spacer" />
      <Link to="/watches/new" class="btn sm primary">+ watch</Link>
      <button
        class="btn sm"
        disabled={reload.isPending}
        onClick={() => reload.mutate(undefined)}
      >
        {reload.isPending ? 'reloading...' : 'reload'}
      </button>
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
