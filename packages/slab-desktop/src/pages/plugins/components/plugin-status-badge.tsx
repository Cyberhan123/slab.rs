import { useTranslation } from '@slab/i18n';

import { cn } from '@/lib/utils';
import type { PluginStatusKey } from '../utils';

export function PluginStatusBadge({ status, busy }: { status: PluginStatusKey; busy?: boolean }) {
  const { t } = useTranslation();
  const normalizedStatus: PluginStatusKey = busy ? 'working' : status;
  const running = normalizedStatus === 'running';
  const invalid = normalizedStatus === 'invalid';

  return (
    <span
      className={cn(
        'rounded-full px-2 py-0.5 text-[10px] font-bold uppercase leading-[15px] tracking-[0.05em]',
        running
          ? 'bg-[color-mix(in_oklab,var(--brand-teal)_20%,var(--shell-card))] text-[var(--brand-teal)]'
          : invalid
            ? 'bg-[var(--status-danger-bg)] text-destructive'
            : 'bg-[#e6e8ea] text-[#3d4947] dark:bg-[var(--surface-soft)] dark:text-muted-foreground',
      )}
    >
      {t(`pages.plugins.status.${normalizedStatus}`)}
    </span>
  );
}
