const PATHS: Record<string, string> = {
  check: '<path d="M3.5 8.5l3 3 6-7"/>',
  x: '<path d="M4 4l8 8M12 4l-8 8"/>',
  alert: '<path d="M8 2.5l6 11H2l6-11zM8 7v3M8 11.6v.01"/>',
  ai: '<path d="M8 2.5l1.4 3.6L13 7.5l-3.6 1.4L8 12.5 6.6 8.9 3 7.5l3.6-1.4L8 2.5z"/>',
  lock: '<rect x="3.5" y="7.5" width="9" height="6" rx="1"/><path d="M5.5 7.5V6a2.5 2.5 0 015 0v1.5"/>',
  trash: '<path d="M3.5 4.5h9M6.5 4.5V3.5a1 1 0 011-1h1a1 1 0 011 1v1M5 4.5l.5 8a1 1 0 001 .9h3a1 1 0 001-.9l.5-8"/>',
  plus: '<path d="M8 3.5v9M3.5 8h9"/>',
  chevron: '<path d="M4 6l4 4 4-4"/>',
  bolt: '<path d="M8.5 2L4 9h3l-.5 5L11 7H8l.5-5z"/>',
  clock: '<circle cx="8" cy="8" r="5.5"/><path d="M8 5v3l2 1.3"/>',
  target: '<circle cx="8" cy="8" r="5.5"/><circle cx="8" cy="8" r="2"/>',
  search: '<circle cx="7" cy="7" r="3.8"/><path d="M10 10l3 3"/>',
  pause: '<path d="M6 4v8M10 4v8"/>',
  globe: '<circle cx="8" cy="8" r="5.5"/><path d="M2.5 8h11M8 2.5c1.8 2 1.8 9 0 11M8 2.5c-1.8 2-1.8 9 0 11"/>',
  edit: '<path d="M10.5 3l2.5 2.5L6 12.5 3 13l.5-3L10.5 3z"/>',
};

interface Props {
  name: string;
  class?: string;
}

export function Icon(props: Props) {
  return (
    <svg
      class={'ic' + (props.class ? ' ' + props.class : '')}
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.5"
      stroke-linecap="round"
      stroke-linejoin="round"
      innerHTML={PATHS[props.name] ?? ''}
    />
  );
}
