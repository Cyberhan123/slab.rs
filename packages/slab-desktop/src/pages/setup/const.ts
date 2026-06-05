import type { components } from '@slab/api/v1';
import { clamp } from 'lodash-es';
import {
  normalizeTaskProgress,
  type NormalizedTaskProgress,
} from '@/pages/task/utils';

export const TASK_POLL_INTERVAL_MS = 1_000;
export const MIN_ACTIVE_PROGRESS = 6;
export const SETUP_ACTIVE_TONE = '#00685f';
export const SETUP_CTA_GRADIENT =
  'linear-gradient(166.52deg, #00685f 0%, #008378 100%)';

export type SetupStatus = components['schemas']['SetupStatusResponse'];
export type OperationAccepted = components['schemas']['OperationAcceptedResponse'];
export type TaskRecord = components['schemas']['TaskResponse'];
export type TaskProgress = components['schemas']['TaskProgressResponse'];
export { normalizeTaskProgress, type NormalizedTaskProgress };

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
) {
  const progress = normalizeTaskProgress(task?.progress);
  if (progress?.label?.trim()) {
    return progress.label.trim();
  }

  switch (state) {
    case 'failed':
      return 'Setup failed';
    case 'succeeded':
      return 'Setup finished';
    case 'starting':
      return runtimePayloadInstalled ? 'Checking desktop prerequisites' : 'Starting setup';
    case 'running':
      return runtimePayloadInstalled ? 'Verifying installed runtime' : 'Preparing Slab runtime';
    default:
      return runtimePayloadInstalled ? 'Checking desktop environment' : 'Preparing environment';
  }
}

export function getProvisionStageHint(
  state: ProvisionState,
  task: TaskRecord | null,
  runtimePayloadInstalled = false,
) {
  const progress = normalizeTaskProgress(task?.progress);
  if (progress?.step && progress.stepCount) {
    return `Step ${progress.step} of ${progress.stepCount}`;
  }

  switch (state) {
    case 'failed':
      return runtimePayloadInstalled
        ? 'Review the error below, then retry the local prerequisite check.'
        : 'Review the error below, then retry the setup task.';
    case 'succeeded':
      return runtimePayloadInstalled
        ? 'FFmpeg and local runtime checks are complete. Launching Slab now.'
        : 'Runtime payloads are in place. Launching Slab now.';
    case 'starting':
      return runtimePayloadInstalled
        ? 'Inspecting the installed runtime and checking whether FFmpeg is already available.'
        : 'Creating the setup task and connecting to the local host.';
    case 'running':
      return runtimePayloadInstalled
        ? 'Checking FFmpeg runtime availability and confirming local workers are ready.'
        : 'Downloading payloads, verifying CABs, checking FFmpeg, and restarting runtime workers.';
    default:
      return runtimePayloadInstalled
        ? 'Inspecting the local desktop installation and FFmpeg availability.'
        : 'Inspecting the local desktop installation.';
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
) {
  if (state === 'failed') {
    return runtimePayloadInstalled
      ? 'Desktop prerequisite checks stopped before setup could complete.'
      : 'Provisioning stopped before setup could complete.';
  }

  if (state === 'succeeded') {
    return '100% complete';
  }

  const progress = normalizeTaskProgress(task?.progress);
  if (progress?.total && progress.total > 0) {
    const percentage = Math.round((progress.current / progress.total) * 100);
    return `${percentage}% complete`;
  }

  if (progress?.step && progress.stepCount) {
    return `Stage ${progress.step}/${progress.stepCount}`;
  }

  if (state === 'starting') {
    return runtimePayloadInstalled ? 'Checking installed runtime...' : 'Creating setup task...';
  }

  if (state === 'running') {
    return runtimePayloadInstalled
      ? 'Checking FFmpeg and local workers...'
      : 'Waiting for progress updates...';
  }

  return 'Waiting to begin';
}
