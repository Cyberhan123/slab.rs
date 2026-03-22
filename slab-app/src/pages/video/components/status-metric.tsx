type StatusMetricProps = {
  label: string;
  value: string;
};

export function StatusMetric({ label, value }: StatusMetricProps) {
  return (
    <div className="space-y-1.5">
      <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
        {label}
      </p>
      <p className="text-sm font-semibold text-foreground">{value}</p>
    </div>
  );
}
