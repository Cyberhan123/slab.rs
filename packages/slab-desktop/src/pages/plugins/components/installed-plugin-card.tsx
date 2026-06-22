import {
  AlertCircle,
  Loader2,
  PlugZap,
  RefreshCw,
  Power,
  Trash2,
  Square,
  type LucideIcon,
} from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import { Button } from '@slab/components/button';
import { cn } from '@/lib/utils';
import { ErrorDataDetail } from '@/components/error-data-detail';
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
  actionError,
  onPrimaryAction,
  onToggleEnabled,
  onUpdate,
  onDelete,
}: {
  plugin: PluginRecord;
  icon: LucideIcon;
  tone: PluginTone;
  busy: boolean;
  actionError: { message: string; error: unknown } | null;
  onPrimaryAction: () => void;
  onToggleEnabled: () => void;
  onUpdate: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const running = isPluginRunning(plugin);
  let primaryActionKey: 'stop' | 'enable' | 'launch' = 'launch';
  if (running) {
    primaryActionKey = 'stop';
  } else if (!plugin.enabled) {
    primaryActionKey = 'enable';
  }
  const primaryLabel = t(`pages.plugins.actions.${primaryActionKey}`);
  let PrimaryIcon = PlugZap;
  if (running) {
    PrimaryIcon = Square;
  } else if (!plugin.enabled) {
    PrimaryIcon = Power;
  }

  let status: PluginStatusKey = 'disabled';
  if (!plugin.valid) {
    status = 'invalid';
  } else if (running) {
    status = 'running';
  } else if (plugin.enabled) {
    status = 'idle';
  }
  const summary = pluginSummaryMessage(plugin);

  let primaryVariant: 'secondary' | 'pill' | 'cta' = 'cta';
  if (running) {
    primaryVariant = 'secondary';
  } else if (!plugin.enabled) {
    primaryVariant = 'pill';
  }

  return (
    <article
      className="relative flex min-h-[194px] flex-col gap-4 rounded-[12px] border border-[color-mix(in_oklab,var(--border)_54%,transparent)] bg-[var(--shell-card)] p-4 transition hover:-translate-y-0.5 hover:border-[color-mix(in_oklab,var(--brand-teal)_28%,var(--border))]"
      data-testid={`plugin-card-${plugin.id}`}
    >
      <div className="flex items-start justify-between gap-3">
        <div className={cn('flex size-10 items-center justify-center rounded-[8px]', toneSurfaceClassName(tone))}>
          <Icon className={cn('size-[19px]', toneTextClassName(tone))} />
        </div>
        <PluginStatusBadge status={status} busy={busy} />
      </div>

      <div className="min-w-0">
        <h3 className="truncate text-base font-bold leading-6 tracking-tight text-foreground">
          {plugin.name}
        </h3>
        <p className="mt-1 line-clamp-2 text-xs leading-4 text-muted-foreground">
          {summary.raw ?? t(summary.key, summary.options)}
        </p>
      </div>

      {plugin.lastError || actionError ? (
        <div className="rounded-[10px] bg-[var(--status-danger-bg)] px-2.5 py-2 text-caption leading-4 text-destructive">
          <div className="flex items-center gap-1.5 font-semibold">
            <AlertCircle className="size-3.5" />
            {t('pages.plugins.card.runtimeIssue')}
          </div>
          <p className="mt-1 line-clamp-2">{actionError?.message ?? plugin.lastError}</p>
          <ErrorDataDetail error={actionError?.error} />
        </div>
      ) : null}

      <div className="mt-auto flex flex-wrap items-center gap-2 pt-2">
        {plugin.updateAvailable ? (
          <Button
            variant="pill"
            size="sm"
            disabled={busy}
            className="h-8 rounded-[8px] text-xs font-bold"
            onClick={onUpdate}
            data-testid={`plugin-update-${plugin.id}`}
          >
            <RefreshCw className="size-3.5" />
            {t('pages.plugins.actions.update')}
          </Button>
        ) : null}
        <Button
          variant={primaryVariant}
          size="sm"
          disabled={busy || (!plugin.valid && !plugin.enabled)}
          className={cn(
            'h-8 flex-1 rounded-[8px] text-xs font-bold',
            !running && plugin.enabled && 'bg-[linear-gradient(135deg,var(--brand-teal)_0%,color-mix(in_oklab,var(--brand-teal)_88%,var(--surface-1))_100%)] text-[color:var(--brand-teal-foreground)]',
          )}
          onClick={onPrimaryAction}
          data-testid={`plugin-primary-action-${plugin.id}`}
        >
          {busy ? <Loader2 className="size-3.5 animate-spin" /> : <PrimaryIcon className="size-3.5" />}
          {primaryLabel}
        </Button>
        <Button
          variant="secondary"
          size="icon-xs"
          className="size-8 rounded-[8px] text-[color:var(--brand-teal)]"
          onClick={onToggleEnabled}
          disabled={busy}
          aria-label={
            plugin.enabled
              ? t('pages.plugins.actions.disableAria', { name: plugin.name })
              : t('pages.plugins.actions.enableAria', { name: plugin.name })
          }
          data-testid={`plugin-toggle-enabled-${plugin.id}`}
        >
          <Power className="size-3.5" />
        </Button>
        {plugin.removable ? (
          <Button
            variant="secondary"
            size="icon-xs"
            className="size-8 rounded-[8px] text-destructive"
            onClick={onDelete}
            disabled={busy}
            aria-label={t('pages.plugins.actions.uninstallAria', { name: plugin.name })}
            data-testid={`plugin-delete-${plugin.id}`}
          >
            <Trash2 className="size-3.5" />
          </Button>
        ) : null}
      </div>
    </article>
  );
}
