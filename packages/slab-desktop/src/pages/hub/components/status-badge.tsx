import { cn } from '@/lib/utils';
import { StatusPill } from '@slab/components/workspace';
import { useTranslation } from '@slab/i18n';

import type { ModelStatus } from '../hooks/use-hub-model-catalog';

export function StatusBadge({
  status,
  className,
}: {
  status: ModelStatus;
  className?: string;
}) {
  const { t } = useTranslation();
  const sharedClassName =
    'px-3 py-1 text-[10px] font-bold uppercase tracking-[0.16em] shadow-none';

  if (status === 'ready') {
    return (
      <StatusPill status="success" className={cn(sharedClassName, 'text-[var(--success)]', className)}>
        {t('pages.hub.filters.statuses.ready')}
      </StatusPill>
    );
  }

  if (status === 'downloading') {
    return (
      <StatusPill status="info" className={cn(sharedClassName, 'text-[var(--primary)]', className)}>
        {t('pages.hub.filters.statuses.downloading')}
      </StatusPill>
    );
  }

  if (status === 'error') {
    return (
      <StatusPill status="danger" className={cn(sharedClassName, 'text-destructive', className)}>
        {t('pages.hub.filters.statuses.error')}
      </StatusPill>
    );
  }

  return (
    <StatusPill status="neutral" className={cn(sharedClassName, 'text-muted-foreground', className)}>
      {t('pages.hub.filters.statuses.not_downloaded')}
    </StatusPill>
  );
}
