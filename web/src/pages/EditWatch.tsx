import { Show } from 'solid-js';
import { useParams, useNavigate } from '@tanstack/solid-router';
import { BuilderPane } from '../components/BuilderPane';
import { createTargetsQuery } from '../lib/queries';

export function EditWatch() {
  const params = useParams({ from: '/watches/$id/edit' });
  const navigate = useNavigate();
  const targets = createTargetsQuery();

  const target = () => (targets.data ?? []).find((t) => t.target_id === params()?.id);

  return (
    <Show
      when={target()}
      fallback={
        <Show
          when={targets.isPending}
          fallback={
            <div style="padding: 16px; color: var(--faint); font-size: 13px">
              Target not found.
            </div>
          }
        >
          <div style="padding: 16px; color: var(--faint); font-size: 13px">Loading...</div>
        </Show>
      }
    >
      {(t) => (
        <BuilderPane
          target={t()}
          onSaved={() => navigate({ to: '/watches/$id', params: { id: t().target_id } })}
          onCancel={() => navigate({ to: '/watches/$id', params: { id: t().target_id } })}
          onDeleted={() => navigate({ to: '/' })}
        />
      )}
    </Show>
  );
}
