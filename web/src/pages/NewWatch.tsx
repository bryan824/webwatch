import { useNavigate } from '@tanstack/solid-router';
import { BuilderPane } from '../components/BuilderPane';

export function NewWatch() {
  const navigate = useNavigate();

  return (
    <BuilderPane
      onSaved={() => navigate({ to: '/' })}
      onCancel={() => navigate({ to: '/' })}
    />
  );
}
