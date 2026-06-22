import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "./lib/utils"

const badgeVariants = cva(
  "focus-ring inline-flex items-center justify-center rounded-full border border-transparent px-2 py-0.5 text-xs font-medium w-fit whitespace-nowrap shrink-0 [&>svg]:size-3 gap-1 [&>svg]:pointer-events-none aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive transition-[color,box-shadow] duration-[var(--dur-180)] ease-out-expo overflow-hidden",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground [a&]:hover:bg-primary/90",
        secondary:
          "bg-secondary text-secondary-foreground [a&]:hover:bg-secondary/90",
        destructive:
          "bg-destructive text-destructive-foreground [a&]:hover:bg-destructive/90 dark:bg-destructive/70",
        outline:
          "border-border text-foreground [a&]:hover:bg-accent [a&]:hover:text-accent-foreground",
        ghost: "[a&]:hover:bg-accent [a&]:hover:text-accent-foreground",
        link: "text-primary underline-offset-4 [a&]:hover:underline",
        chip:
          "border-border/70 bg-[var(--surface-soft)] px-2.5 py-1 text-caption text-muted-foreground shadow-[inset_0_1px_0_color-mix(in_oklab,var(--foreground)_4%,transparent)]",
        counter:
          "border-border/60 bg-[var(--surface-1)] px-2.5 py-1 text-caption font-semibold text-foreground shadow-elevation-1",
        status:
          "border-transparent px-2.5 py-1 text-caption font-semibold",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Badge({
  className,
  variant = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot.Root : "span"

  return (
    <Comp
      data-slot="badge"
      data-variant={variant}
      className={cn(
        badgeVariants({ variant }),
        variant === "status" &&
          "bg-[var(--status-neutral-bg)] text-foreground [&[data-status=success]]:bg-[var(--status-success-bg)] [&[data-status=info]]:bg-[var(--status-info-bg)] [&[data-status=danger]]:bg-[var(--status-danger-bg)]",
        className
      )}
      {...props}
    />
  )
}

export { Badge, badgeVariants }
