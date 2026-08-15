import { getStatusTone } from '../utils';

type Translate = (key: string, options?: Record<string, unknown>) => string;

export function renderStatusPill(status: string, t: Translate) {
  const tone = getStatusTone(status, t);

  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-caption font-bold uppercase tracking-eyebrow ${tone.className}`}
    >
      <span className={`size-1.5 rounded-full ${tone.dotClassName}`} />
      {tone.label}
    </span>
  );
}
