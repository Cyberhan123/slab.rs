import { useState, type ComponentProps } from 'react';

import { Badge } from '@slab/components/badge';
import api from '@slab/api';

const OFFLINE_FAILURE_THRESHOLD = 3;

type StatusBadgeOptions = {
  label: string,
  dotClassName: string,
  variant?: ComponentProps<typeof Badge>['variant'],
  status?: 'neutral' | 'success' | 'danger' | 'info',
  busy?: boolean,
  onClick?: () => void,
};

function statusBadge({
  busy = false,
  dotClassName,
  label,
  onClick,
  status = 'neutral',
  variant = 'status',
}: StatusBadgeOptions) {
  const content = (
    <>
      <span className={dotClassName} />
      {label}
    </>
  );

  return (
    <Badge
      asChild
      variant={variant}
      data-status={status}
      className="gap-1.5 px-3 py-1.5"
    >
      {onClick ? (
        <button
          type="button"
          aria-label={label}
          aria-live="polite"
          aria-atomic="true"
          aria-busy={busy || undefined}
          onClick={onClick}
        >
          {content}
        </button>
      ) : (
        <output aria-live="polite" aria-atomic="true" aria-busy={busy || undefined}>
          {content}
        </output>
      )}
    </Badge>
  );
}

export function BackendStatus() {
  const {
    data,
    dataUpdatedAt,
    error,
    errorUpdatedAt,
    isLoading,
    refetch,
  } = api.useQuery(
    'get',
    '/health',
    {},
    {
      refetchInterval: 30000,
      refetchIntervalInBackground: true,
      // Health is already polled on a fixed interval; global retry would only
      // add duplicate probes and make the status threshold harder to reason about.
      retry: false,
    },
  );
  const [consecutiveFailures, setConsecutiveFailures] = useState(0);
  // Count each react-query transition exactly once, deriving during render
  // (the React-docs adjust-state pattern) instead of a setState-in-effect.
  const [lastObservedUpdate, setLastObservedUpdate] = useState(0);
  if (!isLoading) {
    const updatedAt = Math.max(dataUpdatedAt, errorUpdatedAt);
    if (updatedAt !== 0 && updatedAt !== lastObservedUpdate) {
      setLastObservedUpdate(updatedAt);
      setConsecutiveFailures(error || !data ? consecutiveFailures + 1 : 0);
    }
  }
  const isChecking = isLoading;
  const isOffline = !isChecking && consecutiveFailures >= OFFLINE_FAILURE_THRESHOLD;
  const isOnline = !isChecking && Boolean(data) && !isOffline;

  if (isChecking) {
    return statusBadge({
      label: 'Checking...',
      dotClassName: 'h-2 w-2 rounded-full bg-yellow-500 animate-pulse',
      status: 'info',
      busy: true,
    });
  }

  if (isOnline) {
    return statusBadge({
      label: 'Online',
      dotClassName: 'h-2 w-2 rounded-full bg-green-500',
      status: 'success',
    });
  }

  if (!isOffline) {
    return statusBadge({
      label: 'Unknown',
      dotClassName: 'h-2 w-2 rounded-full bg-muted-foreground',
    });
  }

  return statusBadge({
    label: 'Offline',
    dotClassName: 'h-2 w-2 rounded-full bg-red-500',
    status: 'danger',
    onClick: () => void refetch(),
  });
}
