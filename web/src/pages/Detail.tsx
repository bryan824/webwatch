import { Show, createSignal } from 'solid-js';
import { useParams, useNavigate } from '@tanstack/solid-router';
import { WatchDetail } from '../components/WatchDetail';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { createTargetChecksQuery, createTargetsQuery } from '../lib/queries';
import {
  createCheckNowMutation,
  createDeleteTargetMutation,
  createSetEnabledMutation,
} from '../lib/mutations';

export function Detail() {
  const params = useParams({ from: '/watches/$id' });
  const navigate = useNavigate();
  const targets = createTargetsQuery();
  const check = createCheckNowMutation();
  const del = createDeleteTargetMutation();
  const toggle = createSetEnabledMutation();
  const checks = createTargetChecksQuery(() => params()?.id);
  const [confirmDelete, setConfirmDelete] = createSignal(false);

  const target = () => (targets.data ?? []).find((t) => t.target_id === params()?.id);

  return (
    <Show
      when={target()}
      fallback={
        <Show
          when={targets.isPending}
          fallback={
            <div style="padding: 16px; color: var(--faint); font-size: 13px">
              Target <code class="mono">{params()?.id}</code> not found.
            </div>
          }
        >
          <div style="padding: 16px; color: var(--faint); font-size: 13px">Loading...</div>
        </Show>
      }
    >
      {(t) => (
        <>
          <WatchDetail
            target={t()}
            checking={check.isPending}
            mutating={del.isPending || toggle.isPending}
            onCheckNow={() => check.mutate(t().target_id)}
            onToggleEnabled={() =>
              toggle.mutate({ id: t().target_id, enabled: !t().enabled })
            }
            onDelete={() => setConfirmDelete(true)}
            checks={checks.data ?? []}
          />
          <ConfirmDialog
            open={confirmDelete()}
            onOpenChange={setConfirmDelete}
            title={`Delete ${t().name}?`}
            description="Removes the target and its check history. This cannot be undone."
            confirmLabel={del.isPending ? 'deleting...' : 'delete'}
            confirmDisabled={del.isPending}
            variant="danger"
            onConfirm={() =>
              del.mutate(t().target_id, { onSuccess: () => navigate({ to: '/' }) })
            }
          />
        </>
      )}
    </Show>
  );
}
