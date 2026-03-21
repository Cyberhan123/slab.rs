import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const textareaVariants = cva(
  "placeholder:text-muted-foreground flex field-sizing-content min-h-16 w-full border px-3 py-2 text-base transition-[color,box-shadow,background-color,border-color] outline-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
  {
    variants: {
      variant: {
        default:
          "border-input rounded-md bg-transparent shadow-xs focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] dark:bg-input/30",
        soft:
          "rounded-[20px] border-border/70 bg-[var(--surface-soft)] shadow-[inset_0_1px_0_color-mix(in_oklab,var(--foreground)_4%,transparent)] focus-visible:border-ring focus-visible:ring-ring/40 focus-visible:ring-[3px]",
        shell:
          "rounded-[24px] border-border/60 bg-[var(--surface-input)] shadow-[0_20px_36px_-30px_color-mix(in_oklab,var(--foreground)_32%,transparent)] focus-visible:border-ring focus-visible:ring-ring/35 focus-visible:ring-[3px]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Textarea({
  className,
  variant = "default",
  ...props
}: React.ComponentProps<"textarea"> & VariantProps<typeof textareaVariants>) {
  return (
    <textarea
      data-slot="textarea"
      data-variant={variant}
      className={cn(
        textareaVariants({ variant }),
        "aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
        className
      )}
      {...props}
    />
  )
}

export { Textarea, textareaVariants }
