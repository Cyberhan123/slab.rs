import * as React from "react"

import { cn } from "@/lib/utils"

function Textarea({
  className,
  variant,
  ...props
}: React.ComponentProps<"textarea"> & {
  variant?: "default" | "panel" | "soft" | "surface"
}) {
  return (
    <textarea
      data-slot="textarea"
      data-variant={variant}
      className={cn(
        "flex field-sizing-content min-h-16 w-full rounded-md border border-input bg-transparent px-3 py-2 text-base shadow-xs transition-[color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-destructive/20 md:text-sm dark:bg-input/30 dark:aria-invalid:ring-destructive/40",
        variant === "panel" &&
          "border-border/60 bg-card",
        variant === "soft" &&
          "border-border/60 bg-secondary shadow-none",
        variant === "surface" &&
          "border-border/60 bg-card",
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
