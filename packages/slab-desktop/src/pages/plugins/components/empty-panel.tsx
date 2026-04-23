import type { LucideIcon } from 'lucide-react';

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
    <div className="flex min-h-[160px] flex-col items-center justify-center rounded-[24px] border border-dashed border-border/70 bg-[var(--shell-card)]/45 px-6 py-8 text-center shadow-[0_18px_44px_-38px_color-mix(in_oklab,var(--foreground)_24%,transparent)]">
      <div className="mb-4 flex size-12 items-center justify-center rounded-2xl bg-[var(--surface-soft)] text-muted-foreground">
        <Icon className="size-5" />
      </div>
      <p className="font-medium text-foreground">{title}</p>
      <p className="mt-1 max-w-md text-sm leading-6 text-muted-foreground">{description}</p>
    </div>
  );
}
