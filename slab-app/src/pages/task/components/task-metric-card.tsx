import type { ReactNode } from 'react';
import type { ListChecks } from 'lucide-react';

type TaskMetricCardProps = {
  label: string;
  value: string;
  note: string;
  noteTone: 'success' | 'danger' | 'muted';
  icon: typeof ListChecks;
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
      ? 'text-[#059669]'
      : noteTone === 'danger'
        ? 'text-[#ef4444]'
        : 'text-[#6d7a77]';

  return (
    <article
      className={`rounded-2xl bg-[#f2f4f6] px-6 py-6 shadow-[0_12px_40px_-24px_rgba(25,28,30,0.08)] ${className ?? ''}`}
    >
      <div className="flex items-start justify-between gap-4">
        <p className="text-[12px] font-bold uppercase tracking-[0.14em] text-[#6d7a77]">
          {label}
        </p>
        <Icon className="h-[18px] w-[18px] text-[#5b6872]" />
      </div>
      <div className="mt-5 flex items-end gap-3">
        <p className="text-[30px] font-semibold leading-none tracking-[-0.03em] text-[#191c1e]">
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
