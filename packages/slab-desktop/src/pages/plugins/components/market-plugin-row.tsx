import { Download, Loader2, Star, type LucideIcon } from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import { Button } from '@slab/components/button';
import { marketRating, marketSize, type PluginMarketRecord } from '../utils';

export function MarketPluginRow({
  plugin,
  icon: Icon,
  busy,
  onInstall,
}: {
  plugin: PluginMarketRecord;
  icon: LucideIcon;
  busy: boolean;
  onInstall: () => void;
}) {
  const { t } = useTranslation();
  const versionAction = plugin.installedVersion && plugin.updateAvailable
    ? t('pages.plugins.actions.update')
    : t('pages.plugins.actions.install');
  const actionLabel = plugin.installedVersion && !plugin.updateAvailable
    ? t('pages.plugins.actions.installed')
    : versionAction;

  return (
    <article className="flex items-center justify-between gap-4 rounded-[16px] border border-[color-mix(in_oklab,var(--border)_42%,transparent)] bg-[color-mix(in_oklab,var(--shell-card)_58%,transparent)] p-[17px] shadow-[0_18px_42px_-34px_color-mix(in_oklab,var(--foreground)_28%,transparent)] transition hover:-translate-y-0.5 hover:border-[color-mix(in_oklab,var(--brand-teal)_24%,var(--border))] hover:bg-[var(--shell-card)]/75 hover:shadow-[0_24px_54px_-38px_color-mix(in_oklab,var(--foreground)_34%,transparent)]">
      <div className="flex min-w-0 items-center gap-4">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-[var(--surface-soft)] text-muted-foreground">
          <Icon className="size-5" />
        </div>
        <div className="min-w-0">
          <h3 className="truncate text-base font-medium leading-6 text-foreground">{plugin.name}</h3>
          <p className="truncate text-xs leading-4 text-muted-foreground">
            {plugin.description || t('pages.plugins.market.fallbackDescription', {
              id: plugin.id,
              version: plugin.version,
            })}
          </p>
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-6">
        <div className="hidden text-right sm:block">
          <div className="flex items-center justify-end gap-1 text-xs font-bold text-[var(--brand-gold)]">
            <Star className="size-3 fill-current" />
            {marketRating(plugin)}
          </div>
          <p className="font-mono text-[10px] uppercase tracking-[-0.05em] text-muted-foreground">
            {marketSize(plugin)}
          </p>
        </div>
        <Button
          variant="cta"
          size="sm"
          className="h-7 rounded-[12px] bg-[var(--brand-teal)] px-4 text-xs font-bold"
          onClick={onInstall}
          disabled={busy || Boolean(plugin.installedVersion && !plugin.updateAvailable)}
        >
          {busy ? <Loader2 className="size-3.5 animate-spin" /> : <Download className="size-3.5" />}
          {actionLabel}
        </Button>
      </div>
    </article>
  );
}
