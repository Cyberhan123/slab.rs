import { cn } from '@/lib/utils';
import { Label } from '@slab/components/label';

export function FieldLabel({
  className,
  ...props
}: React.ComponentProps<typeof Label>) {
  return (
    <Label
      className={cn(
        'text-caption font-bold uppercase tracking-eyebrow text-muted-foreground',
        className,
      )}
      {...props}
    />
  );
}
