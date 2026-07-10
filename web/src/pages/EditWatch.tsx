import { Show } from 'solid-js';
import { useParams, useNavigate } from '@tanstack/solid-router';
import { BuilderPane } from '../components/BuilderPane';
import { createTargetDetailQuery } from '../lib/queries';

export function EditWatch() {
  const params = useParams({ from: '/watches/$id/edit' });
  const navigate = useNavigate();
  const detail = createTargetDetailQuery(() => params()?.id);

  return (
    <Show
      when={detail.data?.config}
      fallback={
        <Show
          when={detail.isPending}
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
      {(config) => (
        <BuilderPane
          target={config()}
          onSaved={() => navigate({ to: '/watches/$id', params: { id: config().id } })}
          onCancel={() => navigate({ to: '/watches/$id', params: { id: config().id } })}
          onDeleted={() => navigate({ to: '/' })}
        />
      )}
    </Show>
  );
}
