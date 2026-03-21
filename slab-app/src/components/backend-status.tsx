import { useEffect, useState } from 'react';
import { Badge } from '@/components/ui/badge';
import { checkBackendStatus } from '@/lib/tauri-api';

export function BackendStatus() {
  const [isOnline, setIsOnline] = useState<boolean | null>(null);
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    const checkStatus = async () => {
      setIsChecking(true);
      const status = await checkBackendStatus();
      setIsOnline(status);
      setIsChecking(false);
    };

    // Check immediately
    checkStatus();

    // Check every 30 seconds
    const interval = setInterval(checkStatus, 30000);

    return () => clearInterval(interval);
  }, []);

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
    return statusBadge('Unknown', 'h-2 w-2 rounded-full bg-gray-500');
  }

  if (isOnline) {
    return statusBadge('Online', 'h-2 w-2 rounded-full bg-green-500', 'status', 'success');
  }

  return statusBadge('Offline', 'h-2 w-2 rounded-full bg-red-500', 'status', 'danger');
}
