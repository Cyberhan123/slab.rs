export const API_BASE_URL =
  (import.meta.env.VITE_API_BASE_URL as string | undefined) ??
  'http://localhost:3000';

export const SAMPLE_METHODS = [
  { value: 'auto', label: 'Auto' },
  { value: 'euler', label: 'Euler' },
  { value: 'euler_a', label: 'Euler A' },
  { value: 'heun', label: 'Heun' },
  { value: 'dpm2', label: 'DPM2' },
  { value: 'dpm++2s_a', label: 'DPM++ 2S a' },
  { value: 'dpm++2m', label: 'DPM++ 2M' },
  { value: 'dpm++2mv2', label: 'DPM++ 2M v2' },
  { value: 'lcm', label: 'LCM' },
  { value: 'ipndm', label: 'iPNDM' },
  { value: 'ipndm_v', label: 'iPNDM V' },
] as const;

export const SCHEDULERS = [
  { value: 'auto', label: 'Auto' },
  { value: 'discrete', label: 'Discrete' },
  { value: 'karras', label: 'Karras' },
  { value: 'exponential', label: 'Exponential' },
  { value: 'ays', label: 'AYS' },
  { value: 'gits', label: 'GITS' },
] as const;

export const POLL_INTERVAL_MS = 2_000;
export const MAX_POLL_ATTEMPTS = 150;

export const DIMENSION_PRESETS = [
  { label: '1:1', width: 512, height: 512 },
  { label: '4:3', width: 768, height: 576 },
  { label: '16:9', width: 1024, height: 576 },
] as const;

export const SIDEBAR_LABEL_CLASSNAME =
  'text-[12px] font-semibold leading-4 text-[#191c1e]';
export const SIDEBAR_INPUT_CLASSNAME =
  'h-10 w-full rounded-xl border-[#dbe4ea] bg-white px-3 text-sm text-[#191c1e] shadow-none focus-visible:border-[#64c3ba] focus-visible:ring-[3px] focus-visible:ring-[#0d9488]/12';
export const SIDEBAR_TEXTAREA_CLASSNAME =
  'w-full rounded-xl border-[#dbe4ea] bg-white px-4 py-3 text-sm leading-5 text-[#191c1e] shadow-none resize-none focus-visible:border-[#64c3ba] focus-visible:ring-[3px] focus-visible:ring-[#0d9488]/12';

export type GeneratedImage = {
  src: string;
  prompt: string;
  width: number;
  height: number;
  mode: 'txt2img' | 'img2img';
};

export type TaskResult = {
  image?: string;
  images?: string[];
};

export type ImageRouteState = {
  prompt?: string;
};
