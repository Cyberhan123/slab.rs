import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const inputVariants = cva(
  "file:text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground h-9 w-full min-w-0 rounded-md border px-3 py-1 text-base transition-[color,box-shadow,background-color,border-color] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
  {
    variants: {
      variant: {
        default:
          "border-input bg-transparent shadow-xs focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] dark:bg-input/30",
        soft:
          "rounded-2xl border-border/70 bg-[var(--surface-soft)] shadow-[inset_0_1px_0_color-mix(in_oklab,var(--foreground)_4%,transparent)] focus-visible:border-ring focus-visible:ring-ring/40 focus-visible:ring-[3px]",
        shell:
          "rounded-full border-border/60 bg-[var(--surface-input)] shadow-[0_16px_32px_-28px_color-mix(in_oklab,var(--foreground)_35%,transparent)] focus-visible:border-ring focus-visible:ring-ring/35 focus-visible:ring-[3px]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Input({
  className,
  type,
  variant = "default",
  ...props
}: React.ComponentProps<"input"> & VariantProps<typeof inputVariants>) {
  return (
    <input
      type={type}
      data-slot="input"
      data-variant={variant}
      className={cn(
        inputVariants({ variant }),
        "aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
        className
      )}
      {...props}
    />
  )
}

export { Input, inputVariants }
