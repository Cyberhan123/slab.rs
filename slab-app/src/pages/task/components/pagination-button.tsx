import type { ButtonHTMLAttributes } from 'react';

type PaginationButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  active?: boolean;
};

export function PaginationButton({ active = false, className, ...props }: PaginationButtonProps) {
  return (
    <button
      type="button"
      className={[
        'flex size-8 items-center justify-center rounded-xl text-xs font-bold transition-colors',
        active
          ? 'bg-[var(--brand-teal)] text-white'
          : 'text-[#191c1e] hover:bg-[rgba(0,104,95,0.08)] disabled:text-[#94a3b8] disabled:hover:bg-transparent',
        className,
      ].join(' ')}
      {...props}
    />
  );
}
