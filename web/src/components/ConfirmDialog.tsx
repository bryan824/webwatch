import { Dialog as BDialog } from '@msviderok/base-ui-solid';
import type { JSX } from 'solid-js';

interface Props {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  title: string;
  description: string;
  confirmLabel?: string;
  confirmDisabled?: boolean;
  onConfirm: () => void;
  variant?: 'danger' | 'default';
}

export function ConfirmDialog(props: Props) {
  return (
    <BDialog.Root open={props.open} onOpenChange={props.onOpenChange}>
      <BDialog.Portal>
        <BDialog.Backdrop class="ww-backdrop" />
        <BDialog.Popup class="ww-dialog">
          <h3>{props.title}</h3>
          <p>{props.description}</p>
          <div class="ww-dialog__actions">
            <button class="btn ghost" onClick={() => props.onOpenChange(false)}>
              Cancel
            </button>
            <button
              class={`btn ${props.variant === 'danger' ? 'danger' : 'primary'}`}
              disabled={props.confirmDisabled}
              onClick={() => {
                props.onConfirm();
                props.onOpenChange(false);
              }}
            >
              {props.confirmLabel ?? 'Confirm'}
            </button>
          </div>
        </BDialog.Popup>
      </BDialog.Portal>
    </BDialog.Root>
  );
}
