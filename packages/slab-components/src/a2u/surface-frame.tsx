import type { ComponentProps, ComponentType, ReactNode } from "react"

import { Button } from "../button"
import { cn } from "../lib/utils"
import { StateSurface } from "../state-surface"

type SurfaceVariant = ComponentProps<typeof StateSurface>["variant"]

export type A2uSurfaceAction = {
  disabled?: boolean
  icon?: ComponentType<{ className?: string }>
  label: ReactNode
  onClick: () => void
  testId?: string
  variant?: ComponentProps<typeof Button>["variant"]
}

type A2uSurfaceFrameProps = ComponentProps<"div"> & {
  actions?: A2uSurfaceAction[]
  description?: ReactNode
  icon?: ComponentType<{ className?: string }>
  title: ReactNode
  variant?: SurfaceVariant
}

export function A2uSurfaceFrame({
  actions,
  className,
  description,
  icon,
  title,
  variant = "success",
  ...props
}: A2uSurfaceFrameProps) {
  return (
    <StateSurface
      action={
        actions?.length ? (
          <div className="flex flex-wrap items-center justify-center gap-2">
            {actions.map((action) => {
              const Icon = action.icon

              return (
                <Button
                  key={String(action.label)}
                  type="button"
                  variant={action.variant ?? "pill"}
                  size="sm"
                  onClick={action.onClick}
                  disabled={action.disabled}
                  data-testid={action.testId}
                >
                  {Icon ? <Icon className="size-4" /> : null}
                  {action.label}
                </Button>
              )
            })}
          </div>
        ) : null
      }
      className={cn(
        "border border-border/60 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--surface-1)_94%,transparent),color-mix(in_oklab,var(--surface-soft)_80%,transparent))] px-5 py-8",
        className
      )}
      description={description}
      icon={icon}
      size="compact"
      title={title}
      variant={variant}
      {...props}
    />
  )
}
