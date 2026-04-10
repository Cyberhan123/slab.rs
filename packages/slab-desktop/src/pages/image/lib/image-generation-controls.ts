import {
  getModelConfigFieldValue,
  type ModelConfigDocumentResponse,
} from '@/lib/model-config';
import { SAMPLE_METHODS, SCHEDULERS } from '../const';

export type ImageGenerationMode = 'txt2img' | 'img2img';

export type ImageGenerationControls = {
  mode: ImageGenerationMode;
  widthStr: string;
  heightStr: string;
  numImages: number;
  advancedOpen: boolean;
  cfgScale: number;
  guidance: number;
  steps: number;
  seed: number;
  sampleMethod: string;
  scheduler: string;
  clipSkip: number;
  eta: number;
  strength: number;
};

type UnknownRecord = Record<string, unknown>;

const DEFAULT_IMAGE_GENERATION_CONTROLS: ImageGenerationControls = {
  mode: 'txt2img',
  widthStr: '512',
  heightStr: '512',
  numImages: 1,
  advancedOpen: false,
  cfgScale: 7,
  guidance: 3.5,
  steps: 20,
  seed: -1,
  sampleMethod: 'auto',
  scheduler: 'auto',
  clipSkip: 0,
  eta: 0,
  strength: 0.75,
};

const ALLOWED_IMAGE_COUNTS = new Set([1, 2, 4]);
const SAMPLE_METHOD_VALUES = new Set(SAMPLE_METHODS.map((item) => item.value));
const SCHEDULER_VALUES = new Set(SCHEDULERS.map((item) => item.value));

export function createDefaultImageGenerationControls(): ImageGenerationControls {
  return { ...DEFAULT_IMAGE_GENERATION_CONTROLS };
}

export function normalizeImageGenerationControls(
  value?: Partial<ImageGenerationControls> | null,
): ImageGenerationControls {
  const next = value ?? {};

  return {
    mode: next.mode === 'img2img' ? 'img2img' : DEFAULT_IMAGE_GENERATION_CONTROLS.mode,
    widthStr: normalizeDimensionString(
      next.widthStr,
      DEFAULT_IMAGE_GENERATION_CONTROLS.widthStr,
    ),
    heightStr: normalizeDimensionString(
      next.heightStr,
      DEFAULT_IMAGE_GENERATION_CONTROLS.heightStr,
    ),
    numImages: normalizeImageCount(next.numImages),
    advancedOpen: typeof next.advancedOpen === 'boolean'
      ? next.advancedOpen
      : DEFAULT_IMAGE_GENERATION_CONTROLS.advancedOpen,
    cfgScale: normalizeFiniteNumber(next.cfgScale, DEFAULT_IMAGE_GENERATION_CONTROLS.cfgScale, {
      min: 0,
    }),
    guidance: normalizeFiniteNumber(next.guidance, DEFAULT_IMAGE_GENERATION_CONTROLS.guidance, {
      min: 0,
    }),
    steps: normalizeSafeInteger(next.steps, DEFAULT_IMAGE_GENERATION_CONTROLS.steps, { min: 1 }),
    seed: normalizeSafeInteger(next.seed, DEFAULT_IMAGE_GENERATION_CONTROLS.seed),
    sampleMethod: normalizeKnownString(
      next.sampleMethod,
      SAMPLE_METHOD_VALUES,
      DEFAULT_IMAGE_GENERATION_CONTROLS.sampleMethod,
    ),
    scheduler: normalizeKnownString(
      next.scheduler,
      SCHEDULER_VALUES,
      DEFAULT_IMAGE_GENERATION_CONTROLS.scheduler,
    ),
    clipSkip: normalizeSafeInteger(next.clipSkip, DEFAULT_IMAGE_GENERATION_CONTROLS.clipSkip, {
      min: 0,
    }),
    eta: normalizeFiniteNumber(next.eta, DEFAULT_IMAGE_GENERATION_CONTROLS.eta, {
      min: 0,
    }),
    strength: normalizeFiniteNumber(next.strength, DEFAULT_IMAGE_GENERATION_CONTROLS.strength, {
      min: 0,
      max: 1,
    }),
  };
}

export function areImageGenerationControlsEqual(
  left: Partial<ImageGenerationControls> | null | undefined,
  right: Partial<ImageGenerationControls> | null | undefined,
): boolean {
  const normalizedLeft = normalizeImageGenerationControls(left);
  const normalizedRight = normalizeImageGenerationControls(right);

  return (
    normalizedLeft.mode === normalizedRight.mode &&
    normalizedLeft.widthStr === normalizedRight.widthStr &&
    normalizedLeft.heightStr === normalizedRight.heightStr &&
    normalizedLeft.numImages === normalizedRight.numImages &&
    normalizedLeft.advancedOpen === normalizedRight.advancedOpen &&
    normalizedLeft.cfgScale === normalizedRight.cfgScale &&
    normalizedLeft.guidance === normalizedRight.guidance &&
    normalizedLeft.steps === normalizedRight.steps &&
    normalizedLeft.seed === normalizedRight.seed &&
    normalizedLeft.sampleMethod === normalizedRight.sampleMethod &&
    normalizedLeft.scheduler === normalizedRight.scheduler &&
    normalizedLeft.clipSkip === normalizedRight.clipSkip &&
    normalizedLeft.eta === normalizedRight.eta &&
    normalizedLeft.strength === normalizedRight.strength
  );
}

export function buildImageGenerationControlsFromModelConfig(
  document: ModelConfigDocumentResponse,
): ImageGenerationControls {
  const inferenceSpec = extractResolvedInferenceSpec(document);

  return normalizeImageGenerationControls({
    mode: toImageMode(inferenceSpec.mode),
    widthStr: toDimensionString(inferenceSpec.width),
    heightStr: toDimensionString(inferenceSpec.height),
    numImages: toImageCount(inferenceSpec.n),
    cfgScale: toFiniteNumber(inferenceSpec.cfg_scale),
    guidance: toFiniteNumber(inferenceSpec.guidance),
    steps: toSafeInteger(inferenceSpec.steps),
    seed: toSafeInteger(inferenceSpec.seed),
    sampleMethod: toKnownString(inferenceSpec.sample_method, SAMPLE_METHOD_VALUES),
    scheduler: toKnownString(inferenceSpec.scheduler, SCHEDULER_VALUES),
    clipSkip: toSafeInteger(inferenceSpec.clip_skip),
    eta: toFiniteNumber(inferenceSpec.eta),
    strength: toFiniteNumber(inferenceSpec.strength),
  });
}

function extractResolvedInferenceSpec(document: ModelConfigDocumentResponse): UnknownRecord {
  if (isRecord(document.resolved_inference_spec)) {
    return document.resolved_inference_spec;
  }

  const fallback = getModelConfigFieldValue(document, 'advanced.resolved_inference_spec');
  return isRecord(fallback) ? fallback : {};
}

function normalizeDimensionString(
  value: string | undefined,
  fallback: string,
) {
  if (typeof value !== 'string') {
    return fallback;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return fallback;
  }

  const parsed = Number.parseInt(trimmed, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 64 || parsed > 2048) {
    return fallback;
  }

  return String(parsed);
}

function normalizeImageCount(value: number | undefined) {
  return ALLOWED_IMAGE_COUNTS.has(value ?? Number.NaN)
    ? (value as number)
    : DEFAULT_IMAGE_GENERATION_CONTROLS.numImages;
}

function normalizeSafeInteger(
  value: number | undefined,
  fallback: number,
  options?: { min?: number; max?: number },
) {
  if (!Number.isSafeInteger(value)) {
    return fallback;
  }

  if (options?.min !== undefined && (value as number) < options.min) {
    return fallback;
  }

  if (options?.max !== undefined && (value as number) > options.max) {
    return fallback;
  }

  return value as number;
}

function normalizeFiniteNumber(
  value: number | undefined,
  fallback: number,
  options?: { min?: number; max?: number },
) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return fallback;
  }

  if (options?.min !== undefined && value < options.min) {
    return fallback;
  }

  if (options?.max !== undefined && value > options.max) {
    return fallback;
  }

  return value;
}

function normalizeKnownString(
  value: string | undefined,
  allowedValues: Set<string>,
  fallback: string,
) {
  return typeof value === 'string' && allowedValues.has(value) ? value : fallback;
}

function toImageMode(value: unknown): ImageGenerationMode | undefined {
  return value === 'img2img' || value === 'txt2img' ? value : undefined;
}

function toDimensionString(value: unknown) {
  return typeof value === 'number' && Number.isSafeInteger(value) ? String(value) : undefined;
}

function toImageCount(value: unknown) {
  return typeof value === 'number' && Number.isSafeInteger(value) ? value : undefined;
}

function toSafeInteger(value: unknown) {
  return typeof value === 'number' && Number.isSafeInteger(value) ? value : undefined;
}

function toFiniteNumber(value: unknown) {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

function toKnownString(value: unknown, allowedValues: Set<string>) {
  return typeof value === 'string' && allowedValues.has(value) ? value : undefined;
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
