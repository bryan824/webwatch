import type { StatusKind } from '../lib/status';

interface Props {
  kind: StatusKind;
  glow?: boolean;
  pulse?: boolean;
}

export function StatusDot(props: Props) {
  return (
    <span
      class={`dot ${props.kind}${props.pulse ? ' pulse' : ''}`}
      data-glow={props.glow !== false ? '' : undefined}
    />
  );
}
