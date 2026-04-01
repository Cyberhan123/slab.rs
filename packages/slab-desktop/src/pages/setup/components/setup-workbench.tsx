import type { ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';
import {
  ArrowRight,
  Check,
  Download,
  Loader2,
  RefreshCw,
  TriangleAlert,
} from 'lucide-react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import Header from '@/layouts/header';

import {
  SETUP_ACTIVE_TONE,
  SETUP_CTA_GRADIENT,
  type DependencyTone,
  getDependencyMeta,
} from '../const';
import type { SetupViewModel } from '../hooks/use-setup';

interface DependencyAction {
  label: string;
  icon: LucideIcon;
  onClick: () => Promise<void>;
}

interface DependencyItemView {
  key: string;
  title: string;
  subtitle: string;
  icon: LucideIcon;
  tone: DependencyTone;
  progress: number;
  helperText: string;
  percentLabel: string;
  badgeLabel?: string;
  action?: DependencyAction;
  subtle?: boolean;
}

function SetupScaffold({ children }: { children: ReactNode }) {
  return (
    <div className="h-screen overflow-hidden bg-app-canvas">
      <div className="mx-auto flex h-full w-full flex-col bg-surface-1">
        <Header variant="minimal" />
        <div className="min-h-0 flex-1 overflow-auto">
          {children}
        </div>
      </div>
    </div>
  );
}

function SetupStateCard({
  icon: Icon,
  title,
  description,
  action,
}: {
  icon: LucideIcon;
  title: string;
  description: ReactNode;
  action?: ReactNode;
}) {
  return (
    <SetupScaffold>
      <div className="flex min-h-full items-center justify-center px-6 py-10">
        <div className="w-full max-w-lg rounded-2xl border border-border/40 bg-surface-1 p-8 shadow-[0px_12px_40px_-12px_rgba(25,28,30,0.08)]">
          <div className="flex size-12 items-center justify-center rounded-xl bg-surface-soft text-foreground">
            <Icon className="size-5" />
          </div>
          <div className="mt-6 space-y-2">
            <h1 className="text-xl font-semibold text-foreground">{title}</h1>
            <div className="text-sm leading-6 text-muted-foreground">{description}</div>
          </div>
          {action ? <div className="mt-6">{action}</div> : null}
        </div>
      </div>
    </SetupScaffold>
  );
}

function DependencyBadge({
  tone,
  label,
}: {
  tone: Extract<DependencyTone, 'success' | 'active'>;
  label: string;
}) {
  const Icon = tone === 'success' ? Check : Loader2;

  return (
    <div
      className={cn(
        'inline-flex items-center gap-2 rounded-full px-3 py-1',
        tone === 'success'
          ? 'bg-[#00685f]/10 text-[#00685f]'
          : 'bg-[#00685f]/5 text-[#00685f]/70',
      )}
    >
      <Icon className={cn('size-3', tone === 'active' && 'animate-spin')} />
      <span className="text-[11px] font-bold uppercase tracking-[0.12em]">
        {label}
      </span>
    </div>
  );
}

function DependencyActionButton({ action }: { action: DependencyAction }) {
  const Icon = action.icon;

  return (
    <Button
      type="button"
      size="sm"
      variant="outline"
      className="h-8 rounded-full border-0 bg-white/70 px-4 text-[11px] font-semibold text-[#00685f] shadow-none hover:bg-white"
      onClick={() => {
        void action.onClick();
      }}
    >
      <Icon className="size-3.5" />
      {action.label}
    </Button>
  );
}

function DependencyRow({ item }: { item: DependencyItemView }) {
  const Icon = item.icon;
  const helperToneClass =
    item.tone === 'error'
      ? 'text-destructive'
      : item.tone === 'idle'
        ? 'text-muted-foreground'
        : 'text-[#00685f]';
  const progressTrackClass =
    item.tone === 'error'
      ? 'bg-destructive/15'
      : item.tone === 'idle'
        ? 'bg-muted-foreground/10'
        : 'bg-[#00685f]/10';
  const progressFillClass =
    item.tone === 'error'
      ? 'bg-destructive'
      : item.tone === 'idle'
        ? 'bg-muted-foreground/25'
        : 'bg-[#00685f]';

  return (
    <div
      className={cn(
        'rounded-xl bg-surface-soft p-5 md:p-[21px]',
        item.subtle && 'opacity-70',
        item.tone === 'error' && 'ring-1 ring-destructive/15',
      )}
    >
      <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div className="min-w-0 flex-1">
          <div className="flex items-start gap-4">
            <div
              className={cn(
                'mt-0.5 flex size-12 shrink-0 items-center justify-center rounded-xl bg-surface-input',
                item.tone === 'error'
                  ? 'text-destructive'
                  : item.tone === 'idle'
                    ? 'text-muted-foreground'
                    : 'text-[#00685f]',
              )}
            >
              <Icon className="size-5" />
            </div>

            <div className="min-w-0 flex-1">
              <div className="text-base font-semibold text-foreground">
                {item.title}
              </div>
              <div className="mt-0.5 text-xs leading-4 text-secondary-foreground">
                {item.subtitle}
              </div>

              <div className="mt-3 space-y-1">
                <div className="flex items-center justify-between gap-3">
                  <span
                    className={cn(
                      'min-w-0 text-[10px] font-medium leading-[15px]',
                      helperToneClass,
                    )}
                  >
                    {item.helperText}
                  </span>
                  <span
                    className={cn(
                      'shrink-0 text-[10px] font-medium leading-[15px]',
                      helperToneClass,
                    )}
                  >
                    {item.percentLabel}
                  </span>
                </div>
                <div
                  className={cn(
                    'h-1.5 w-full overflow-hidden rounded-full',
                    progressTrackClass,
                  )}
                >
                  <div
                    className={cn(
                      'h-full rounded-full transition-[width] duration-300 ease-out',
                      progressFillClass,
                    )}
                    style={{ width: `${item.progress}%` }}
                  />
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="flex shrink-0 justify-end md:pl-4">
          {item.action ? <DependencyActionButton action={item.action} /> : null}
          {!item.action && item.badgeLabel ? (
            <DependencyBadge
              tone={item.tone === 'active' ? 'active' : 'success'}
              label={item.badgeLabel}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
}

export function SetupWorkbench({
  status,
  isChecking,
  checkError,
  ffmpegDownload,
  ffmpegError,
  ffmpegProgress,
  ffmpegReady,
  allBackendsUnavailable,
  completing,
  saveError,
  handleDownloadFfmpeg,
  handleComplete,
}: SetupViewModel) {
  if (isChecking || (!status && !checkError)) {
    return (
      <SetupStateCard
        icon={Loader2}
        title="Checking environment"
        description="Inspecting your local runtime before the setup wizard continues."
        action={
          <div className="flex items-center gap-3 text-sm text-muted-foreground">
            <Loader2 className="size-4 animate-spin" />
            <span>Please wait a moment.</span>
          </div>
        }
      />
    );
  }

  if (checkError) {
    return (
      <SetupStateCard
        icon={TriangleAlert}
        title="Could not reach the server"
        description={
          <>
            <p>{checkError}</p>
            <p className="mt-2">
              Make sure <code>slab-server</code> is running, then try again.
            </p>
          </>
        }
        action={
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              window.location.reload();
            }}
          >
            <RefreshCw className="size-4" />
            Reload
          </Button>
        }
      />
    );
  }

  if (!status) {
    return null;
  }

  const ffmpegMeta = getDependencyMeta('ffmpeg');

  const ffmpegItem: DependencyItemView =
    ffmpegDownload === 'downloading'
      ? {
          key: 'ffmpeg',
          title: ffmpegMeta.title,
          subtitle: ffmpegMeta.subtitle,
          icon: ffmpegMeta.icon,
          tone: 'active',
          progress: ffmpegProgress,
          helperText: 'Downloading binaries...',
          percentLabel: `${Math.round(ffmpegProgress)}%`,
          badgeLabel: 'Active',
        }
      : ffmpegReady
        ? {
            key: 'ffmpeg',
            title: ffmpegMeta.title,
            subtitle: ffmpegMeta.subtitle,
            icon: ffmpegMeta.icon,
            tone: 'success',
            progress: 100,
            helperText: 'Installation complete',
            percentLabel: '100%',
            badgeLabel: 'Installed',
          }
        : ffmpegDownload === 'error'
          ? {
              key: 'ffmpeg',
              title: ffmpegMeta.title,
              subtitle: ffmpegMeta.subtitle,
              icon: ffmpegMeta.icon,
              tone: 'error',
              progress: 0,
              helperText: ffmpegError ?? 'Download failed',
              percentLabel: '0%',
              action: {
                label: 'Retry',
                icon: RefreshCw,
                onClick: handleDownloadFfmpeg,
              },
            }
          : {
              key: 'ffmpeg',
              title: ffmpegMeta.title,
              subtitle: ffmpegMeta.subtitle,
              icon: ffmpegMeta.icon,
              tone: 'idle',
              progress: 0,
              helperText: ffmpegMeta.idleLabel,
              percentLabel: '0%',
              action: {
                label: 'Download',
                icon: Download,
                onClick: handleDownloadFfmpeg,
              },
            };

  const dependencyItems: DependencyItemView[] = [
    ffmpegItem,
    ...status.backends.map((backend): DependencyItemView => {
      const meta = getDependencyMeta(backend.name);

      if (backend.installed) {
        return {
          key: backend.name,
          title: meta.title,
          subtitle: meta.subtitle,
          icon: meta.icon,
          tone: 'success',
          progress: 100,
          helperText: 'Installation complete',
          percentLabel: '100%',
          badgeLabel: 'Installed',
        };
      }

      return {
        key: backend.name,
        title: meta.title,
        subtitle: meta.subtitle,
        icon: meta.icon,
        tone: 'idle',
        progress: 0,
        helperText: meta.idleLabel,
        percentLabel: '0%',
        subtle: true,
      };
    }),
  ];

  const hasFooterNotes = allBackendsUnavailable || Boolean(saveError);

  return (
    <SetupScaffold>
      <main className="mx-auto flex min-h-full w-full max-w-[944px] flex-col gap-10 px-6 py-8">
        <div className="max-w-[576px]">
          <p className="text-lg leading-[29px] text-secondary-foreground">
            Slab needs a few dependencies before it can process AI workloads.
          </p>
        </div>

        <section className="flex h-[min(720px,calc(100vh-8rem))] min-h-[560px] flex-col overflow-hidden rounded-xl border border-border/40 bg-surface-1 shadow-[0px_12px_40px_-12px_rgba(25,28,30,0.06)]">
          <div className="flex min-h-0 flex-1 flex-col gap-6 p-6 md:p-8">
            <div className="shrink-0">
              <div>
                <p className="text-[13px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                  System Dependencies
                </p>
              </div>
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto pr-4">
              <div className="space-y-4">
                {dependencyItems.map((item) => (
                  <DependencyRow key={item.key} item={item} />
                ))}
              </div>
            </div>
          </div>

          <footer
            className={cn(
              'mt-auto flex flex-col gap-4 border-t border-border/20 px-6 py-5 md:px-8',
              hasFooterNotes
                ? 'md:flex-row md:items-center md:justify-between'
                : 'md:flex-row md:items-center md:justify-end',
            )}
          >
            <div className="space-y-1">
              {allBackendsUnavailable ? (
                <p className="max-w-[560px] text-sm leading-6 text-muted-foreground">
                  AI backends can be added later from <strong>Settings -&gt; Backends</strong>.
                  You can continue now and use cloud-provider mode in the meantime.
                </p>
              ) : null}

              {saveError ? (
                <p className="text-sm leading-6 text-destructive">{saveError}</p>
              ) : null}
            </div>

            <Button
              type="button"
              size="lg"
              className="h-12 w-full rounded-xl px-8 text-base font-semibold text-white shadow-[0px_10px_15px_-3px_rgba(0,0,0,0.10),0px_4px_6px_-4px_rgba(0,0,0,0.10)] hover:brightness-[1.03] sm:w-auto"
              style={{
                backgroundColor: SETUP_ACTIVE_TONE,
                backgroundImage: SETUP_CTA_GRADIENT,
              }}
              disabled={completing}
              onClick={() => {
                void handleComplete();
              }}
            >
              {completing ? (
                <>
                  <Loader2 className="size-4 animate-spin" />
                  Saving...
                </>
              ) : (
                <>
                  Continue to App
                  <ArrowRight className="size-4" />
                </>
              )}
            </Button>
          </footer>
        </section>
      </main>
    </SetupScaffold>
  );
}
