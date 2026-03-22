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
        'flex size-10 items-center justify-center rounded-2xl text-slate-600 transition',
        active && 'bg-white text-[#00685f] shadow-[0_12px_24px_-18px_rgba(15,23,42,0.45)]',
        !active && 'hover:bg-white/70 hover:text-slate-900',
        disabled && 'cursor-not-allowed opacity-35 hover:bg-transparent hover:text-slate-600',
      )}
    >
      <Icon className="h-[18px] w-[18px]" />
    </button>
  );
}
