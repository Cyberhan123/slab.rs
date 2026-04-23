import {
  AlertCircle,
  Loader2,
  PlugZap,
  Power,
  Square,
  type LucideIcon,
} from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import { Button } from '@slab/components/button';
import { cn } from '@/lib/utils';
import {
  isPluginRunning,
  pluginSummaryMessage,
  toneSurfaceClassName,
  toneTextClassName,
  type PluginRecord,
  type PluginStatusKey,
  type PluginTone,
} from '../utils';
import { PluginStatusBadge } from './plugin-status-badge';

export function InstalledPluginCard({
  plugin,
  icon: Icon,
  tone,
  busy,
  onPrimaryAction,
  onToggleEnabled,
}: {
  plugin: PluginRecord;
  icon: LucideIcon;
  tone: PluginTone;
  busy: boolean;
  onPrimaryAction: () => void;
  onToggleEnabled: () => void;
}) {
  const { t } = useTranslation();
  const running = isPluginRunning(plugin);
  const primaryActionKey = running ? 'stop' : !plugin.enabled ? 'enable' : 'launch';
  const primaryLabel = t(`pages.plugins.actions.${primaryActionKey}`);
  const PrimaryIcon = running ? Square : !plugin.enabled ? Power : PlugZap;
  const status: PluginStatusKey = !plugin.valid
    ? 'invalid'
    : running
      ? 'running'
      : plugin.enabled
        ? 'idle'
        : 'disabled';
  const summary = pluginSummaryMessage(plugin);

  return (
    <article className="relative flex min-h-[194px] flex-col gap-4 rounded-[12px] border border-[color-mix(in_oklab,var(--border)_54%,transparent)] bg-[var(--shell-card)] p-[17px] shadow-[var(--shell-elevation)] transition hover:-translate-y-0.5 hover:border-[color-mix(in_oklab,var(--brand-teal)_28%,var(--border))] hover:shadow-[0_24px_50px_-40px_color-mix(in_oklab,var(--foreground)_38%,transparent)]">
      <div className="flex items-start justify-between gap-3">
        <div className={cn('flex size-10 items-center justify-center rounded-[8px]', toneSurfaceClassName(tone))}>
          <Icon className={cn('size-[19px]', toneTextClassName(tone))} />
        </div>
        <PluginStatusBadge status={status} busy={busy} />
      </div>

      <div className="min-w-0">
        <h3 className="truncate text-base font-bold leading-6 tracking-[-0.02em] text-foreground">
          {plugin.name}
        </h3>
        <p className="mt-1 line-clamp-2 text-xs leading-4 text-muted-foreground">
          {summary.raw ?? t(summary.key, summary.options)}
        </p>
      </div>

      {plugin.lastError ? (
        <div className="rounded-[10px] bg-[var(--status-danger-bg)] px-2.5 py-2 text-[11px] leading-4 text-destructive">
          <div className="flex items-center gap-1.5 font-semibold">
            <AlertCircle className="size-3.5" />
            {t('pages.plugins.card.runtimeIssue')}
          </div>
          <p className="mt-1 line-clamp-2">{plugin.lastError}</p>
        </div>
      ) : null}

      <div className="mt-auto flex items-center gap-2 pt-2">
        <Button
          variant={running ? 'secondary' : !plugin.enabled ? 'pill' : 'cta'}
          size="sm"
          disabled={busy || (!plugin.valid && !plugin.enabled)}
          className={cn(
            'h-8 flex-1 rounded-[8px] text-xs font-bold',
            !running && plugin.enabled && 'bg-[linear-gradient(135deg,#00685f_0%,#008378_100%)] text-white',
          )}
          onClick={onPrimaryAction}
        >
          {busy ? <Loader2 className="size-3.5 animate-spin" /> : <PrimaryIcon className="size-3.5" />}
          {primaryLabel}
        </Button>
        <Button
          variant="secondary"
          size="icon-xs"
          className="size-8 rounded-[8px] text-[var(--brand-teal)]"
          onClick={onToggleEnabled}
          disabled={busy}
          aria-label={
            plugin.enabled
              ? t('pages.plugins.actions.disableAria', { name: plugin.name })
              : t('pages.plugins.actions.enableAria', { name: plugin.name })
          }
        >
          <Power className="size-3.5" />
        </Button>
      </div>
    </article>
  );
}
