type Translate = (key: string, options?: { label: string; value?: number }) => string;

export function parseOptionalInt(
  raw: string,
  fieldLabel: string,
  min: number,
  t: Translate,
): number | undefined {
  const trimmed = raw.trim();
  if (!trimmed) return undefined;

  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed)) {
    throw new Error(t('pages.audio.validation.integer', { label: fieldLabel }));
  }
  if (parsed < min) {
    throw new Error(t('pages.audio.validation.min', { label: fieldLabel, value: min }));
  }
  return parsed;
}

export function parseOptionalFloat(
  raw: string,
  fieldLabel: string,
  t: Translate,
  options: { min?: number; max?: number; exclusiveMin?: number } = {},
): number | undefined {
  const trimmed = raw.trim();
  if (!trimmed) return undefined;

  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed)) {
    throw new Error(t('pages.audio.validation.number', { label: fieldLabel }));
  }
  if (options.min !== undefined && parsed < options.min) {
    throw new Error(t('pages.audio.validation.min', { label: fieldLabel, value: options.min }));
  }
  if (options.max !== undefined && parsed > options.max) {
    throw new Error(t('pages.audio.validation.max', { label: fieldLabel, value: options.max }));
  }
  if (options.exclusiveMin !== undefined && parsed <= options.exclusiveMin) {
    throw new Error(
      t('pages.audio.validation.exclusiveMin', {
        label: fieldLabel,
        value: options.exclusiveMin,
      }),
    );
  }
  return parsed;
}
