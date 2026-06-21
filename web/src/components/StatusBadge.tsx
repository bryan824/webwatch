import { deriveStatus } from '../lib/status';
import { StatusDot } from './StatusDot';
import type { TargetStatus } from '../lib/types';

interface Props {
  target: TargetStatus;
}

export function StatusBadge(props: Props) {
  const s = () => deriveStatus(props.target);
  return (
    <div class={`status-badge ${s().kind}`}>
      <StatusDot kind={s().kind} />
      {s().label}
    </div>
  );
}
