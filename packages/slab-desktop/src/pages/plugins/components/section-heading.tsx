import type { ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';

export function SectionHeading({
  icon: Icon,
  title,
  action,
}: {
  icon: LucideIcon;
  title: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="flex items-center gap-2">
        <Icon className="size-5 text-[var(--brand-teal)]" />
        <h2 className="text-xl font-semibold leading-7 tracking-[-0.02em] text-foreground">{title}</h2>
      </div>
      {action}
    </div>
  );
}
