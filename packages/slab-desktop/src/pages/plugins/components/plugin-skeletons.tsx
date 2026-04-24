import { INSTALLED_SKELETON_KEYS } from '../utils';

export function InstalledSkeletonGrid() {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
      {INSTALLED_SKELETON_KEYS.map((key) => (
        <div
          key={key}
          className="min-h-[194px] animate-pulse rounded-[12px] bg-[var(--shell-card)] p-[17px] shadow-[var(--shell-elevation)]"
        >
          <div className="flex items-start justify-between">
            <div className="size-10 rounded-[8px] bg-[var(--surface-soft)]" />
            <div className="h-5 w-14 rounded-full bg-[var(--surface-soft)]" />
          </div>
          <div className="mt-8 h-4 w-28 rounded bg-[var(--surface-soft)]" />
          <div className="mt-3 h-3 w-36 rounded bg-[var(--surface-soft)]" />
          <div className="mt-7 h-8 rounded-[8px] bg-[var(--surface-soft)]" />
        </div>
      ))}
    </div>
  );
}
