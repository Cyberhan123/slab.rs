import type { ComponentProps, ComponentType, ReactNode } from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Ban, CheckCircle2, Inbox, Loader2, TriangleAlert } from "lucide-react"

import { cn } from "./lib/utils"

const stateSurfaceVariants = cva(
  "glass-surface soft-in flex flex-col items-center justify-center gap-5 px-6 py-12 text-center text-balance",
  {
    variants: {
      size: {
        compact: "min-h-[160px] rounded-2xl",
        default: "min-h-[240px] rounded-3xl",
        stage: "min-h-[360px] rounded-3xl",
      },
    },
    defaultVariants: {
      size: "default",
    },
  }
)

const defaultIcons = {
  aborted: Ban,
  empty: Inbox,
  error: TriangleAlert,
  interrupted: Ban,
  loading: Loader2,
  success: CheckCircle2,
} as const

type StateSurfaceProps = ComponentProps<"div"> &
  VariantProps<typeof stateSurfaceVariants> & {
    action?: ReactNode
    description?: ReactNode
    icon?: ComponentType<{ className?: string }>
    title: ReactNode
    variant: keyof typeof defaultIcons
  }

function StateSurface({
  action,
  className,
  description,
  icon,
  size,
  title,
  variant,
  ...props
}: StateSurfaceProps) {
  const Icon = icon ?? defaultIcons[variant]
  const isLoading = variant === "loading"

  return (
    <div
      aria-live={variant === "error" ? "assertive" : "polite"}
      data-slot="state-surface"
      data-variant={variant}
      role={isLoading ? "status" : undefined}
      className={cn(stateSurfaceVariants({ size }), className)}
      {...props}
    >
      <div className="flex size-16 shrink-0 items-center justify-center rounded-2xl bg-glass-bg-strong text-muted-foreground">
        <Icon className={cn("size-7", isLoading && "animate-spin")} />
      </div>
      <div className="space-y-2">
        <h3 className="text-lg font-semibold tracking-tight text-foreground">{title}</h3>
        {description ? (
          <div className="mx-auto max-w-md text-sm leading-6 text-muted-foreground">{description}</div>
        ) : null}
      </div>
      {action ? <div className="mt-1">{action}</div> : null}
    </div>
  )
}

export { StateSurface }
