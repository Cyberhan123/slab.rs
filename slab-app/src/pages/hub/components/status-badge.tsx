import { Badge } from '@/components/ui/badge';

import type { ModelStatus } from '../hooks/use-hub-model-catalog';

export function StatusBadge({ status }: { status: ModelStatus }) {
  if (status === 'ready') {
    return (
      <Badge className="border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300">
        Ready
      </Badge>
    );
  }

  if (status === 'downloading') {
    return (
      <Badge className="border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300">
        Downloading
      </Badge>
    );
  }

  if (status === 'error') {
    return <Badge variant="destructive">Error</Badge>;
  }

  return <Badge variant="outline">Not downloaded</Badge>;
}
