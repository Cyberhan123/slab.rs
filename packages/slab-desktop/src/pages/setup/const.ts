import type { components } from '@slab/api/v1';
import { clamp } from 'lodash-es';
import {
  normalizeTaskProgress,
  type NormalizedTaskProgress,
} from '@/pages/task/utils';

export const TASK_POLL_INTERVAL_MS = 1_000;
export const MIN_ACTIVE_PROGRESS = 6;
export const SETUP_ACTIVE_TONE = 'var(--brand-teal)';
export const SETUP_CTA_GRADIENT =
  'linear-gradient(166.52deg, var(--brand-teal) 0%, color-mix(in oklab, var(--brand-teal) 88%, var(--surface-1)) 100%)';

export type SetupStatus = components['schemas']['SetupStatusResponse'];
export type OperationAccepted = components['schemas']['OperationAcceptedResponse'];
export type TaskRecord = components['schemas']['TaskResponse'];
export type TaskProgress = components['schemas']['TaskProgressResponse'];
export { normalizeTaskProgress, type NormalizedTaskProgress };
type Translate = (key: string, options?: Record<string, unknown>) => string;
export type RuntimePayloadMode = 'bundled' | 'packaged';

export type ProvisionState =
  | 'idle'
  | 'starting'
  | 'running'
  | 'succeeded'
  | 'failed';

export function getProvisionStageLabel(
  state: ProvisionState,
  task: TaskRecord | null,
  runtimePayloadInstalled = false,
  t?: Translate,
) {
  const progress = normalizeTaskProgress(task?.progress, t);
  if (progress?.label?.trim()) {
    return progress.label.trim();
  }

  switch (state) {
    case 'failed':
      return translate(t, 'pages.setup.stages.failed', 'Setup failed');
    case 'succeeded':
      return translate(t, 'pages.setup.stages.finished', 'Setup finished');
    case 'starting':
      return runtimePayloadInstalled
        ? translate(t, 'pages.setup.stages.checkingPrerequisites', 'Checking desktop prerequisites')
        : translate(t, 'pages.setup.stages.starting', 'Starting setup');
    case 'running':
      return runtimePayloadInstalled
        ? translate(t, 'pages.setup.stages.verifyingInstalledRuntime', 'Verifying installed runtime')
        : translate(t, 'pages.setup.stages.preparingRuntime', 'Preparing Slab runtime');
    default:
      return runtimePayloadInstalled
        ? translate(t, 'pages.setup.stages.checkingEnvironment', 'Checking desktop environment')
        : translate(t, 'pages.setup.stages.preparingEnvironment', 'Preparing environment');
  }
}

export function getProvisionStageHint(
  state: ProvisionState,
  task: TaskRecord | null,
  runtimePayloadInstalled = false,
  t?: Translate,
) {
  const progress = normalizeTaskProgress(task?.progress, t);
  if (progress?.message?.trim()) {
    return progress.message.trim();
  }
  if (progress?.step && progress.stepCount) {
    return translate(t, 'pages.setup.hints.step', 'Step {{step}} of {{count}}', {
      count: progress.stepCount,
      step: progress.step,
    });
  }

  switch (state) {
    case 'failed':
      return runtimePayloadInstalled
        ? translate(
            t,
            'pages.setup.hints.failedBundled',
            'Review the error below, then retry the local prerequisite check.',
          )
        : translate(
            t,
            'pages.setup.hints.failedPackaged',
            'Review the error below, then retry the setup task.',
          );
    case 'succeeded':
      return runtimePayloadInstalled
        ? translate(
            t,
            'pages.setup.hints.succeededBundled',
            'FFmpeg and local runtime checks are complete. Launching Slab now.',
          )
        : translate(
            t,
            'pages.setup.hints.succeededPackaged',
            'Runtime payloads are in place. Launching Slab now.',
          );
    case 'starting':
      return runtimePayloadInstalled
        ? translate(
            t,
            'pages.setup.hints.startingBundled',
            'Inspecting the installed runtime and checking whether FFmpeg is already available.',
          )
        : translate(
            t,
            'pages.setup.hints.startingPackaged',
            'Creating the setup task and connecting to the local host.',
          );
    case 'running':
      return runtimePayloadInstalled
        ? translate(
            t,
            'pages.setup.hints.runningBundled',
            'Checking FFmpeg runtime availability and confirming local workers are ready.',
          )
        : translate(
            t,
            'pages.setup.hints.runningPackaged',
            'Downloading payloads, verifying CABs, checking FFmpeg, and restarting runtime workers.',
          );
    default:
      return runtimePayloadInstalled
        ? translate(
            t,
            'pages.setup.hints.idleBundled',
            'Inspecting the local desktop installation and FFmpeg availability.',
          )
        : translate(
            t,
            'pages.setup.hints.idlePackaged',
            'Inspecting the local desktop installation.',
          );
  }
}

export function getProvisionProgressValue(
  state: ProvisionState,
  task: TaskRecord | null,
) {
  if (state === 'succeeded') {
    return 100;
  }

  const progress = normalizeTaskProgress(task?.progress);
  if (progress?.step && progress.stepCount) {
    const currentStep = Math.max(progress.step - 1, 0);
    const stepFraction =
      progress.total && progress.total > 0
        ? clamp(progress.current / progress.total, 0, 1)
        : 0;
    return clamp(((currentStep + stepFraction) / progress.stepCount) * 100, 0, 99);
  }

  if (progress?.total && progress.total > 0) {
    return clamp((progress.current / progress.total) * 100, 0, 99);
  }

  if (state === 'starting' || state === 'running') {
    return MIN_ACTIVE_PROGRESS;
  }

  return 0;
}

export function getProvisionProgressSummary(
  state: ProvisionState,
  task: TaskRecord | null,
  runtimePayloadInstalled = false,
  t?: Translate,
) {
  if (state === 'failed') {
    return runtimePayloadInstalled
      ? translate(
          t,
          'pages.setup.summary.failedBundled',
          'Desktop prerequisite checks stopped before setup could complete.',
        )
      : translate(
          t,
          'pages.setup.summary.failedPackaged',
          'Provisioning stopped before setup could complete.',
        );
  }

  if (state === 'succeeded') {
    return translate(t, 'pages.setup.summary.complete', '100% complete');
  }

  const progress = normalizeTaskProgress(task?.progress);
  if (progress?.total && progress.total > 0) {
    const percentage = Math.round((progress.current / progress.total) * 100);
    return translate(t, 'pages.setup.summary.percentComplete', '{{percentage}}% complete', {
      percentage,
    });
  }

  if (progress?.step && progress.stepCount) {
    return translate(t, 'pages.setup.summary.stage', 'Stage {{step}}/{{count}}', {
      count: progress.stepCount,
      step: progress.step,
    });
  }

  if (state === 'starting') {
    return runtimePayloadInstalled
      ? translate(t, 'pages.setup.summary.startingBundled', 'Checking installed runtime...')
      : translate(t, 'pages.setup.summary.startingPackaged', 'Creating setup task...');
  }

  if (state === 'running') {
    return runtimePayloadInstalled
      ? translate(t, 'pages.setup.summary.runningBundled', 'Checking FFmpeg and local workers...')
      : translate(t, 'pages.setup.summary.runningPackaged', 'Waiting for progress updates...');
  }

  return translate(t, 'pages.setup.summary.idle', 'Waiting to begin');
}

function translate(
  t: Translate | undefined,
  key: string,
  defaultValue: string,
  options: Record<string, unknown> = {},
) {
  const interpolatedDefaultValue = interpolateDefaultValue(defaultValue, options);
  return t ? t(key, { defaultValue: interpolatedDefaultValue, ...options }) : interpolatedDefaultValue;
}

function interpolateDefaultValue(value: string, options: Record<string, unknown>) {
  return Object.entries(options).reduce((nextValue, [key, option]) => {
    return nextValue.replaceAll(`{{${key}}}`, String(option));
  }, value);
}
