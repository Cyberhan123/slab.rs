import { cn } from '@/lib/utils';
import { Label } from '@slab/components/label';

export function FieldLabel({
  className,
  ...props
}: React.ComponentProps<typeof Label>) {
  return (
    <Label
      className={cn(
        'text-[11px] font-bold uppercase tracking-[0.18em] text-muted-foreground',
        className,
      )}
      {...props}
    />
  );
}
