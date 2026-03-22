import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground shadow-[0_10px_28px_-18px_color-mix(in_oklab,var(--primary)_70%,transparent)] hover:bg-primary/92",
        destructive:
          "bg-destructive text-destructive-foreground hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 dark:bg-destructive/70",
        outline:
          "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground dark:bg-input/30 dark:border-input dark:hover:bg-input/50",
        secondary:
          "bg-secondary text-secondary-foreground hover:bg-secondary/80",
        ghost:
          "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
        link: "text-primary underline-offset-4 hover:underline",
        pill:
          "rounded-full border border-border/70 bg-[var(--surface-1)] text-foreground shadow-[0_12px_28px_-24px_color-mix(in_oklab,var(--foreground)_40%,transparent)] hover:bg-[var(--surface-selected)] hover:text-foreground",
        cta:
          "rounded-full bg-[var(--brand-teal)] text-[var(--brand-teal-foreground)] shadow-[0_18px_36px_-18px_color-mix(in_oklab,var(--brand-teal)_55%,transparent)] hover:brightness-[1.04]",
        quiet:
          "rounded-full bg-transparent text-muted-foreground hover:bg-[var(--surface-soft)] hover:text-foreground",
        rail:
          "justify-start rounded-[18px] border border-transparent bg-transparent px-3 text-muted-foreground hover:bg-[var(--surface-soft)] hover:text-foreground data-[active=true]:border-border/70 data-[active=true]:bg-[var(--surface-1)] data-[active=true]:text-foreground data-[active=true]:shadow-[0_12px_28px_-24px_color-mix(in_oklab,var(--foreground)_40%,transparent)]",
      },
      size: {
        default: "h-9 px-4 py-2 has-[>svg]:px-3",
        xs: "h-6 gap-1 rounded-md px-2 text-xs has-[>svg]:px-1.5 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-8 rounded-md gap-1.5 px-3 has-[>svg]:px-2.5",
        lg: "h-10 rounded-md px-6 has-[>svg]:px-4",
        icon: "size-9",
        "icon-xs": "size-6 rounded-md [&_svg:not([class*='size-'])]:size-3",
        "icon-sm": "size-8",
        "icon-lg": "size-10",
        pill: "h-10 rounded-full px-4 text-sm has-[>svg]:px-3",
        rail: "h-11 rounded-[18px] px-3 text-sm has-[>svg]:px-3",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
