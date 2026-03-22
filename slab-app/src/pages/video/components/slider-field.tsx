import type { ReactNode } from 'react';
import { FieldLabel } from './field-label';

type SliderFieldProps = {
  label: string;
  value: string | number;
  slider: ReactNode;
};

export function SliderField({ label, value, slider }: SliderFieldProps) {
  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between">
        <FieldLabel>{label}</FieldLabel>
        <span className="font-mono text-[12px] font-semibold text-[#00685f]">
          {value}
        </span>
      </div>
      {slider}
    </div>
  );
}
