import { getStatusTone } from '../utils';

export function renderStatusPill(status: string) {
  const tone = getStatusTone(status);

  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-[11px] font-bold uppercase tracking-[0.04em] ${tone.className}`}
    >
      <span className={`size-1.5 rounded-full ${tone.dotClassName}`} />
      {tone.label}
    </span>
  );
}
