import { cn } from '@/lib/utils';
import { StatusPill } from '@/components/ui/workspace';

import type { ModelStatus } from '../hooks/use-hub-model-catalog';

export function StatusBadge({
  status,
  className,
}: {
  status: ModelStatus;
  className?: string;
}) {
  const sharedClassName =
    'px-3 py-1 text-[10px] font-bold uppercase tracking-[0.16em] shadow-none';

  if (status === 'ready') {
    return (
      <StatusPill status="success" className={cn(sharedClassName, 'text-[var(--success)]', className)}>
        Ready
      </StatusPill>
    );
  }

  if (status === 'downloading') {
    return (
      <StatusPill status="info" className={cn(sharedClassName, 'text-[var(--primary)]', className)}>
        Downloading
      </StatusPill>
    );
  }

  if (status === 'error') {
    return (
      <StatusPill status="danger" className={cn(sharedClassName, 'text-destructive', className)}>
        Error
      </StatusPill>
    );
  }

  return (
    <StatusPill status="neutral" className={cn(sharedClassName, 'text-muted-foreground', className)}>
      Not downloaded
    </StatusPill>
  );
}
