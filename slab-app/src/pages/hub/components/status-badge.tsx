import { StatusPill } from '@/components/ui/workspace';

import type { ModelStatus } from '../hooks/use-hub-model-catalog';

export function StatusBadge({ status }: { status: ModelStatus }) {
  if (status === 'ready') {
    return (
      <StatusPill status="success" className="text-xs">
        Ready
      </StatusPill>
    );
  }

  if (status === 'downloading') {
    return (
      <StatusPill status="info" className="text-xs">
        Downloading
      </StatusPill>
    );
  }

  if (status === 'error') {
    return (
      <StatusPill status="danger" className="text-xs">
        Error
      </StatusPill>
    );
  }

  return (
    <StatusPill status="neutral" className="text-xs">
      Not downloaded
    </StatusPill>
  );
}
