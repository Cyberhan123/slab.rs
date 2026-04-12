import { Badge } from '@slab/components/badge';
import api from '@/lib/api';

export function BackendStatus() {
  const {
    data,
    error,
    isLoading,
    isRefetching,
  } = api.useQuery(
    'get',
    '/health',
    {},
    {
      refetchInterval: 30000,
      refetchIntervalInBackground: true,
      retry: false,
    },
  );
  const isChecking = isLoading || isRefetching;
  const isOnline = isChecking ? null : Boolean(data && !error);

  const statusBadge = (
    label: string,
    dotClassName: string,
    variant: React.ComponentProps<typeof Badge>['variant'] = 'status',
    status: 'neutral' | 'success' | 'danger' | 'info' = 'neutral',
    busy = false,
  ) => (
    <Badge
      variant={variant}
      data-status={status}
      className="gap-1.5 px-3 py-1.5"
      role="status"
      aria-live="polite"
      aria-atomic="true"
      aria-busy={busy || undefined}
    >
      <div className={dotClassName} />
      {label}
    </Badge>
  );

  if (isChecking) {
    return statusBadge('Checking...', 'h-2 w-2 rounded-full bg-yellow-500 animate-pulse', 'status', 'info', true);
  }

  if (isOnline === null) {
    return statusBadge('Unknown', 'h-2 w-2 rounded-full bg-muted-foreground');
  }

  if (isOnline) {
    return statusBadge('Online', 'h-2 w-2 rounded-full bg-green-500', 'status', 'success');
  }

  return statusBadge('Offline', 'h-2 w-2 rounded-full bg-red-500', 'status', 'danger');
}
