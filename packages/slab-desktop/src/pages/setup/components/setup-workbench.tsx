import type { ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';
import {
  CheckCircle2,
  Loader2,
  RefreshCw,
  TriangleAlert,
} from 'lucide-react';

import { Button } from '@slab/components/button';
import { cn } from '@/lib/utils';
import Header from '@/layouts/header';

import { SETUP_ACTIVE_TONE, SETUP_CTA_GRADIENT } from '../const';
import type { SetupViewModel } from '../hooks/use-setup';

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

function SetupBadge({
  label,
  tone,
}: {
  label: string;
  tone: 'active' | 'success' | 'error';
}) {
  return (
    <div
      className={cn(
        'inline-flex items-center rounded-full px-3 py-1 text-[11px] font-bold uppercase tracking-[0.14em]',
        tone === 'active' && 'bg-[#00685f]/10 text-[#00685f]',
        tone === 'success' && 'bg-[#00685f]/10 text-[#00685f]',
        tone === 'error' && 'bg-destructive/10 text-destructive',
      )}
    >
      {label}
    </div>
  );
}

export function SetupWorkbench({
  setupStatus,
  isChecking,
  checkError,
  provisionState,
  provisionError,
  stageLabel,
  stageHint,
  progressPercent,
  progressSummary,
  canRetry,
  handleRetry,
}: SetupViewModel) {
  if (isChecking) {
    return (
      <SetupStateCard
        icon={Loader2}
        title="Checking desktop environment"
        description="Inspecting the local Slab host, the packaged runtime, and FFmpeg availability."
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
        title="Could not reach the local host"
        description={
          <>
            <p>{checkError}</p>
            <p className="mt-2">
              Make sure <code>slab-server</code> is running, then reload this page.
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

  const isFailed = provisionState === 'failed';
  const isSucceeded = provisionState === 'succeeded';
  const isActive = provisionState === 'starting' || provisionState === 'running';
  const runtimePayloadInstalled = setupStatus?.runtime_payload_installed ?? false;
  const ffmpegInstalled = setupStatus?.ffmpeg.installed ?? false;
  const readyBackends = setupStatus?.backends.filter((backend) => backend.installed).length ?? 0;
  const totalBackends = setupStatus?.backends.length ?? 0;
  const Icon = isFailed ? TriangleAlert : isSucceeded ? CheckCircle2 : Loader2;

  return (
    <SetupScaffold>
      <main className="mx-auto flex min-h-full w-full max-w-6xl items-center px-6 py-8">
        <section className="relative w-full overflow-hidden rounded-[32px] border border-border/40 bg-surface-1 shadow-[0px_18px_60px_-30px_rgba(25,28,30,0.18)]">
          <div className="absolute inset-0 opacity-80 [background:radial-gradient(circle_at_top_left,color-mix(in_oklab,var(--brand-teal)_14%,transparent),transparent_34%),radial-gradient(circle_at_bottom_right,color-mix(in_oklab,var(--brand-gold)_12%,transparent),transparent_28%)]" />

          <div className="relative grid gap-8 p-8 md:p-10 lg:grid-cols-[1.2fr,0.9fr]">
            <div className="space-y-6">
              <div className="inline-flex size-14 items-center justify-center rounded-2xl bg-[color:color-mix(in_oklab,var(--surface-soft)_84%,white)] text-[var(--brand-teal)]">
                <Icon className={cn('size-6', isActive && 'animate-spin')} />
              </div>

              <div className="space-y-4">
                <div className="space-y-2">
                  <p className="text-[12px] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
                    Desktop Setup
                  </p>
                  <h1 className="max-w-2xl text-4xl font-semibold tracking-tight text-foreground md:text-[2.8rem]">
                    {runtimePayloadInstalled
                      ? 'Slab is checking your local tools.'
                      : 'Slab is preparing your local runtime.'}
                  </h1>
                </div>

                <p className="max-w-2xl text-sm leading-7 text-secondary-foreground md:text-base">
                  {runtimePayloadInstalled ? (
                    <>
                      This installation already includes the packaged runtime payload under
                      <code className="mx-1 rounded bg-surface-soft px-1.5 py-0.5 text-[0.9em]">
                        resources/libs
                      </code>
                      . Slab is now checking whether FFmpeg is already available and will install
                      it automatically when needed before continuing.
                    </>
                  ) : (
                    <>
                      Slab will reuse the release CAB payloads for your current version, verify
                      them against the embedded manifest, install the runtime into
                      <code className="mx-1 rounded bg-surface-soft px-1.5 py-0.5 text-[0.9em]">
                        resources/libs
                      </code>
                      , check FFmpeg, and restart the managed runtime workers.
                    </>
                  )}
                </p>

                <div className="grid gap-3 pt-2 sm:grid-cols-3">
                  <div className="rounded-2xl border border-border/40 bg-[color:color-mix(in_oklab,var(--surface-1)_88%,white)] p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
                      Runtime Payload
                    </p>
                    <p className="mt-2 text-sm font-medium text-foreground">
                      {runtimePayloadInstalled ? 'Installed locally' : 'Needs setup'}
                    </p>
                  </div>
                  <div className="rounded-2xl border border-border/40 bg-[color:color-mix(in_oklab,var(--surface-1)_88%,white)] p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
                      FFmpeg
                    </p>
                    <p className="mt-2 text-sm font-medium text-foreground">
                      {ffmpegInstalled ? 'Available' : 'Will be installed'}
                    </p>
                  </div>
                  <div className="rounded-2xl border border-border/40 bg-[color:color-mix(in_oklab,var(--surface-1)_88%,white)] p-4">
                    <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
                      Backend Workers
                    </p>
                    <p className="mt-2 text-sm font-medium text-foreground">
                      {totalBackends > 0 ? `${readyBackends}/${totalBackends} ready` : 'Not reported'}
                    </p>
                  </div>
                </div>
              </div>
            </div>

            <div className="rounded-[28px] border border-border/50 bg-[color:color-mix(in_oklab,var(--surface-1)_88%,white)] p-6 shadow-[0_12px_32px_-24px_rgba(25,28,30,0.18)]">
              <div className="flex items-start justify-between gap-4">
                <div className="space-y-2">
                  <p className="text-[12px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                    Current Stage
                  </p>
                  <h2 className="text-xl font-semibold text-foreground">{stageLabel}</h2>
                  <p className="text-sm leading-6 text-muted-foreground">{stageHint}</p>
                </div>

                <SetupBadge
                  label={isFailed ? 'Failed' : isSucceeded ? 'Complete' : 'Running'}
                  tone={isFailed ? 'error' : isSucceeded ? 'success' : 'active'}
                />
              </div>

              <div className="mt-8 space-y-3">
                <div className="flex items-center justify-between gap-3 text-xs font-medium text-muted-foreground">
                  <span>{progressSummary}</span>
                  <span>{Math.round(progressPercent)}%</span>
                </div>
                <div className="h-2.5 overflow-hidden rounded-full bg-[color:color-mix(in_oklab,var(--surface-soft)_78%,transparent)]">
                  <div
                    className={cn(
                      'h-full rounded-full transition-[width] duration-300 ease-out',
                      isFailed ? 'bg-destructive' : 'bg-[var(--brand-teal)]',
                    )}
                    style={{ width: `${progressPercent}%` }}
                  />
                </div>
              </div>

              <div className="mt-8 border-t border-border/30 pt-6">
                {provisionError ? (
                  <p className="text-sm leading-6 text-destructive">{provisionError}</p>
                ) : (
                  <p className="text-sm leading-6 text-muted-foreground">
                    {isSucceeded
                      ? 'Setup has completed. Slab will enter the application automatically.'
                      : runtimePayloadInstalled
                        ? 'Keep this window open while Slab checks FFmpeg and confirms the packaged runtime is ready.'
                        : 'Keep this window open while the local setup task finishes provisioning the runtime.'}
                  </p>
                )}
              </div>

              <div className="mt-6 flex flex-wrap items-center gap-3">
                {canRetry ? (
                  <Button
                    type="button"
                    size="lg"
                    className="rounded-xl px-6 text-white hover:brightness-[1.03]"
                    style={{
                      backgroundColor: SETUP_ACTIVE_TONE,
                      backgroundImage: SETUP_CTA_GRADIENT,
                    }}
                    onClick={() => {
                      void handleRetry();
                    }}
                  >
                    <RefreshCw className="size-4" />
                    Retry setup
                  </Button>
                ) : (
                  <div className="inline-flex items-center gap-3 rounded-xl bg-surface-soft px-4 py-3 text-sm text-muted-foreground">
                    <Loader2 className={cn('size-4', isActive && 'animate-spin')} />
                    <span>
                      {isSucceeded
                        ? 'Launching Slab...'
                        : runtimePayloadInstalled
                          ? 'Checking desktop prerequisites'
                          : 'Provisioning in progress'}
                    </span>
                  </div>
                )}
              </div>
            </div>
          </div>
        </section>
      </main>
    </SetupScaffold>
  );
}
