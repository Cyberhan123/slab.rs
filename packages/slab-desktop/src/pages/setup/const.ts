import type { LucideIcon } from 'lucide-react';
import {
  BrainCircuit,
  Clapperboard,
  Cpu,
  Image,
  Mic,
  Orbit,
  Package,
  Sparkles,
  Waves,
} from 'lucide-react';

import type { components } from '@/lib/api/v1.d.ts';

export const TASK_POLL_INTERVAL_MS = 2_000;
export const PROGRESS_STEP = 3;
export const PROGRESS_MAX_SIMULATED = 90;
export const SETUP_ACTIVE_TONE = '#00685f';
export const SETUP_CTA_GRADIENT =
  'linear-gradient(166.52deg, #00685f 0%, #008378 100%)';

export type SetupStatus = components['schemas']['SetupStatusResponse'];
export type ComponentStatus = components['schemas']['ComponentStatusResponse'];
export type OperationAccepted = components['schemas']['OperationAcceptedResponse'];
export type TaskRecord = components['schemas']['TaskResponse'];

export type DownloadState = 'idle' | 'downloading' | 'done' | 'error';
export type DependencyTone = 'success' | 'active' | 'idle' | 'error';

export interface DependencyMeta {
  title: string;
  subtitle: string;
  icon: LucideIcon;
  idleLabel: string;
}

export const DEPENDENCY_META: Record<string, DependencyMeta> = {
  ffmpeg: {
    title: 'Ffmpeg',
    subtitle: 'Core Media Engine',
    icon: Clapperboard,
    idleLabel: 'Ready to install',
  },
  'ggml.llama': {
    title: 'Ggml.Llama',
    subtitle: 'Large Language Model Engine',
    icon: BrainCircuit,
    idleLabel: 'Pending...',
  },
  'ggml.whisper': {
    title: 'Ggml.Whisper',
    subtitle: 'Speech-to-Text Model',
    icon: Waves,
    idleLabel: 'Pending...',
  },
  'ggml.diffusion': {
    title: 'Ggml.Diffusion',
    subtitle: 'Image Generation Engine',
    icon: Image,
    idleLabel: 'Pending...',
  },
  'candle.llama': {
    title: 'Candle.Llama',
    subtitle: 'Rust Inference Engine (Text)',
    icon: Cpu,
    idleLabel: 'Waiting...',
  },
  'candle.whisper': {
    title: 'Candle.Whisper',
    subtitle: 'Rust Inference Engine (Audio)',
    icon: Mic,
    idleLabel: 'Waiting...',
  },
  'candle.diffusion': {
    title: 'Candle.Diffusion',
    subtitle: 'Rust Inference Engine (Image)',
    icon: Sparkles,
    idleLabel: 'Queued',
  },
  onnx: {
    title: 'Onnx Runtime',
    subtitle: 'Cross-platform ML Engine',
    icon: Orbit,
    idleLabel: 'Queued',
  },
};

function toTitleCaseSegment(segment: string) {
  return segment ? `${segment[0].toUpperCase()}${segment.slice(1)}` : segment;
}

export function getDependencyMeta(name: string): DependencyMeta {
  const meta = DEPENDENCY_META[name];
  if (meta) {
    return meta;
  }

  return {
    title: name
      .split('.')
      .map((segment) => toTitleCaseSegment(segment))
      .join('.'),
    subtitle: 'Runtime dependency',
    icon: Package,
    idleLabel: 'Pending...',
  };
}
