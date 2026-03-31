import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

type ToolbarIconButtonProps = {
  icon: (props: { className?: string }) => ReactNode;
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
};

export function ToolbarIconButton({
  icon: Icon,
  label,
  active = false,
  disabled = false,
  onClick,
}: ToolbarIconButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        'flex size-10 items-center justify-center rounded-2xl text-muted-foreground transition',
        active && 'bg-[var(--shell-card)] text-[var(--brand-teal)] shadow-[0_12px_24px_-18px_color-mix(in_oklab,var(--foreground)_45%,transparent)]',
        !active && 'hover:bg-[var(--shell-card)]/70 hover:text-foreground',
        disabled && 'cursor-not-allowed opacity-35 hover:bg-transparent hover:text-muted-foreground',
      )}
    >
      <Icon className="h-[18px] w-[18px]" />
    </button>
  );
}
