type StatusMetricProps = {
  label: string;
  value: string;
};

export function StatusMetric({ label, value }: StatusMetricProps) {
  return (
    <div className="space-y-1.5">
      <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-slate-500">
        {label}
      </p>
      <p className="text-sm font-semibold text-slate-900">{value}</p>
    </div>
  );
}
