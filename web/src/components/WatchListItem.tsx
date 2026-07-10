import { Link } from '@tanstack/solid-router';
import { deriveStatus } from '../lib/status';
import { formatRelative } from '../lib/format';
import { StatusDot } from './StatusDot';
import type { TargetStatus } from '../lib/types';

interface Props {
  target: TargetStatus;
  selected: boolean;
}

export function WatchListItem(props: Props) {
  const s = () => deriveStatus(props.target);

  return (
    <Link
      to="/watches/$id"
      params={{ id: props.target.target_id }}
      class="watch"
      aria-current={props.selected ? 'true' : undefined}
    >
      <StatusDot kind={s().kind} glow />
      <div class="watch__body">
        <span class="watch__name truncate">{props.target.name}</span>
        <span class="watch__url truncate">{props.target.url}</span>
      </div>
      <span class="watch__meta">{formatRelative(props.target.last_success_at)}</span>
    </Link>
  );
}
