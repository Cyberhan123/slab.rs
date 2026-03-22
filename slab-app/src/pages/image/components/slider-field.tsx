import type { ReactNode } from 'react';
import { Label } from '@/components/ui/label';
import { SIDEBAR_LABEL_CLASSNAME } from '../const';

type SliderFieldProps = {
  label: string;
  value: string | number;
  slider: ReactNode;
};

export function SliderField({ label, value, slider }: SliderFieldProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <Label className={SIDEBAR_LABEL_CLASSNAME}>{label}</Label>
        <span className="text-[11px] font-medium text-muted-foreground">{value}</span>
      </div>
      {slider}
    </div>
  );
}
