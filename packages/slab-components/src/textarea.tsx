import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "./lib/utils"

const supportsFieldSizingContent = () =>
  typeof CSS !== "undefined" && CSS.supports("field-sizing", "content")

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
  autoResize = false,
  onInput,
  value,
  ...props
}: React.ComponentProps<"textarea"> &
  VariantProps<typeof textareaVariants> & {
    autoResize?: boolean
  }) {
  const textareaRef = React.useRef<HTMLTextAreaElement | null>(null)

  const syncAutoResize = React.useCallback(() => {
    const element = textareaRef.current
    if (!autoResize || !element || supportsFieldSizingContent()) {
      return
    }

    element.style.height = "0px"
    element.style.height = `${element.scrollHeight}px`

    const maxHeight = Number.parseFloat(window.getComputedStyle(element).maxHeight)
    element.style.overflowY =
      Number.isFinite(maxHeight) && element.scrollHeight > maxHeight ? "auto" : "hidden"
  }, [autoResize])

  React.useLayoutEffect(() => {
    syncAutoResize()
  }, [syncAutoResize, value])

  return (
    <textarea
      ref={textareaRef}
      data-slot="textarea"
      data-variant={variant}
      className={cn(
        textareaVariants({ variant }),
        "aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
        className
      )}
      value={value}
      onInput={(event) => {
        syncAutoResize()
        onInput?.(event)
      }}
      {...props}
    />
  )
}

export { Textarea, textareaVariants }
