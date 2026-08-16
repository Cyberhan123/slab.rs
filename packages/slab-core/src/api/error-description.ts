const isErrorRecord = (value: unknown): value is { error?: unknown; message?: unknown } =>
  typeof value === 'object' && value !== null;

export const getErrorDescription = (value: unknown, fallback: string): string => {
  if (value instanceof Error && value.message.trim()) {
    return value.message;
  }

  if (!isErrorRecord(value)) {
    return fallback;
  }

  if (typeof value.message === 'string' && value.message.trim()) {
    return value.message;
  }

  if (typeof value.error === 'string' && value.error.trim()) {
    return value.error;
  }

  return fallback;
};
