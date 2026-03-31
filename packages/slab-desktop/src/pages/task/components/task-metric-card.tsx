import type { ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';

type TaskMetricCardProps = {
  label: string;
  value: string;
  note: string;
  noteTone: 'success' | 'danger' | 'muted';
  icon: LucideIcon;
  className?: string;
  children: ReactNode;
};

export function TaskMetricCard({
  label,
  value,
  note,
  noteTone,
  icon: Icon,
  className,
  children,
}: TaskMetricCardProps) {
  const noteClassName =
    noteTone === 'success'
      ? 'text-success'
      : noteTone === 'danger'
        ? 'text-destructive'
        : 'text-muted-foreground';

  return (
    <article
      className={`rounded-2xl border border-border/50 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--surface-1)_68%,var(--surface-soft))_0%,var(--surface-soft)_100%)] px-6 py-6 shadow-[0_12px_40px_-24px_color-mix(in_oklab,var(--foreground)_14%,transparent)] ${className ?? ''}`}
    >
      <div className="flex items-start justify-between gap-4">
        <p className="text-[12px] font-bold uppercase tracking-[0.14em] text-muted-foreground">
          {label}
        </p>
        <Icon className="h-[18px] w-[18px] text-muted-foreground" />
      </div>
      <div className="mt-5 flex items-end gap-3">
        <p className="text-[30px] font-semibold leading-none tracking-[-0.03em] text-foreground">
          {value}
        </p>
        <p className={`pb-1 text-[12px] font-semibold ${noteClassName}`}>
          {note}
        </p>
      </div>
      {children}
    </article>
  );
}
