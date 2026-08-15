import type { LucideIcon } from 'lucide-react';
import { StateSurface } from '@slab/components/state-surface';

export function EmptyPanel({
  icon: Icon,
  title,
  description,
}: {
  icon: LucideIcon;
  title: string;
  description: string;
}) {
  return (
    <StateSurface
      description={description}
      icon={Icon}
      size="compact"
      title={title}
      variant="empty"
    />
  );
}
