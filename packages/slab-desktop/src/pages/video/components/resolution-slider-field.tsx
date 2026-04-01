import { Slider } from '@slab/components/slider';
import { FieldLabel } from './field-label';

type ResolutionSliderFieldProps = {
  label: string;
  value: string;
  min: number;
  max: number;
  step: number;
  onChange: (value: string) => void;
};

export function ResolutionSliderField({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: ResolutionSliderFieldProps) {
  const numericValue = Number.parseInt(value, 10);
  const resolvedValue = Number.isFinite(numericValue) ? numericValue : min;

  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between">
        <FieldLabel>{label}</FieldLabel>
        <span className="font-mono text-[12px] font-semibold text-primary">
          {resolvedValue}
        </span>
      </div>
      <Slider
        min={min}
        max={max}
        step={step}
        value={[resolvedValue]}
        onValueChange={([nextValue]) => onChange(String(nextValue))}
        className="[&_[data-slot=slider-range]]:bg-primary [&_[data-slot=slider-thumb]]:border-primary [&_[data-slot=slider-track]]:bg-border/70"
      />
    </div>
  );
}
