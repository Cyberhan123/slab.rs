import { SERVER_BASE_URL } from '@/lib/config';

export const API_BASE_URL = SERVER_BASE_URL;

export const SAMPLE_METHODS = [
  { value: 'auto', label: 'Auto' },
  { value: 'euler', label: 'Euler' },
  { value: 'euler_a', label: 'Euler A' },
  { value: 'lcm', label: 'LCM' },
  { value: 'dpm++2m', label: 'DPM++ 2M' },
] as const;

export const SCHEDULERS = [
  { value: 'auto', label: 'Auto' },
  { value: 'discrete', label: 'Discrete' },
  { value: 'karras', label: 'Karras' },
] as const;

export const FRAME_OPTIONS = [8, 16, 24, 32, 48, 60, 80, 120] as const;
export const FPS_OPTIONS = [6, 8, 12, 16, 24, 30, 48, 60] as const;

export const POLL_INTERVAL_MS = 2_000;
export const MAX_POLL_ATTEMPTS = 300;
export const DIFFUSION_BACKEND_ID = 'ggml.diffusion';

export type ModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  local_path: string | null;
};
