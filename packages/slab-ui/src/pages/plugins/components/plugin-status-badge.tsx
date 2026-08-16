import { useTranslation } from '@slab/i18n';

import { cn } from '@slab/ui/lib/utils';
import type { PluginStatusKey } from '../utils';

export function PluginStatusBadge({ status, busy }: { status: PluginStatusKey; busy?: boolean }) {
  const { t } = useTranslation();
  const normalizedStatus: PluginStatusKey = busy ? 'working' : status;
  const running = normalizedStatus === 'running';
  const invalid = normalizedStatus === 'invalid';

  return (
    <span
      className={cn(
        'rounded-full px-2 py-0.5 text-micro font-bold uppercase leading-[15px] tracking-eyebrow',
        running
          ? 'bg-primary/20 text-primary'
          : invalid
            ? 'bg-destructive/15 text-destructive dark:bg-destructive/20'
            : 'bg-secondary text-muted-foreground',
      )}
    >
      {t(`pages.plugins.status.${normalizedStatus}`)}
    </span>
  );
}
