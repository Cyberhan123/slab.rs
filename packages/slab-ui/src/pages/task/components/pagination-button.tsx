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
          ? 'bg-[var(--brand-teal)] text-[color:var(--brand-teal-foreground)]'
          : 'text-foreground hover:bg-[color:color-mix(in_oklab,var(--brand-teal)_8%,transparent)] disabled:text-muted-foreground disabled:hover:bg-transparent',
        className,
      ].join(' ')}
      {...props}
    />
  );
}
