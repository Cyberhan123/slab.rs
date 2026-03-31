import type { ComponentProps, ComponentType, ReactNode } from "react"
import { UploadCloud } from "lucide-react"

import { cn } from "./lib/utils"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"

export function SoftPanel({
  className,
  ...props
}: ComponentProps<"section">) {
  return (
    <section
      className={cn(
        "workspace-soft-panel rounded-[24px] px-5 py-4",
        className
      )}
      {...props}
    />
  )
}

type MetricCardProps = ComponentProps<typeof Card> & {
  label: string
  value: ReactNode
  hint?: ReactNode
  icon?: ComponentType<{ className?: string }>
}

export function MetricCard({
  label,
  value,
  hint,
  icon: Icon,
  className,
  ...props
}: MetricCardProps) {
  return (
    <Card variant="metric" className={cn("gap-4", className)} {...props}>
      <CardHeader className="flex flex-row items-start justify-between gap-4">
        <div className="space-y-1">
          <CardDescription className="text-[11px] font-semibold uppercase tracking-[0.16em]">
            {label}
          </CardDescription>
          <CardTitle className="text-3xl font-semibold tracking-tight">
            {value}
          </CardTitle>
        </div>
        {Icon ? (
          <div className="flex size-11 items-center justify-center rounded-2xl bg-[var(--surface-soft)] text-muted-foreground">
            <Icon className="size-5" />
          </div>
        ) : null}
      </CardHeader>
      {hint ? <CardContent className="pt-0 text-sm text-muted-foreground">{hint}</CardContent> : null}
    </Card>
  )
}

export function StatusPill({
  status = "neutral",
  className,
  ...props
}: ComponentProps<typeof Badge> & {
  status?: "neutral" | "success" | "info" | "danger"
}) {
  return (
    <Badge
      variant="status"
      data-status={status}
      className={cn("px-3 py-1.5", className)}
      {...props}
    />
  )
}

export function PillFilterBar({
  className,
  ...props
}: ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "workspace-soft-panel flex flex-wrap items-center gap-2 rounded-full p-2",
        className
      )}
      {...props}
    />
  )
}

type SplitWorkbenchProps = ComponentProps<"div"> & {
  sidebar: ReactNode
  main: ReactNode
  sidebarClassName?: string
  mainClassName?: string
}

export function SplitWorkbench({
  sidebar,
  main,
  className,
  sidebarClassName,
  mainClassName,
  ...props
}: SplitWorkbenchProps) {
  return (
    <div
      className={cn(
        "grid min-h-0 gap-5 xl:grid-cols-[minmax(320px,380px)_minmax(0,1fr)]",
        className
      )}
      {...props}
    >
      <div className={cn("min-h-0 space-y-5", sidebarClassName)}>{sidebar}</div>
      <div className={cn("min-h-0", mainClassName)}>{main}</div>
    </div>
  )
}

type UploadDropzoneProps = ComponentProps<"button"> & {
  title: string
  description: string
  preview?: ReactNode
  actionLabel?: string
}

export function UploadDropzone({
  title,
  description,
  preview,
  actionLabel = "Select file",
  className,
  type = "button",
  children,
  ...props
}: UploadDropzoneProps) {
  return (
    <button
      type={type}
      className={cn(
        "workspace-soft-panel flex w-full flex-col items-center justify-center gap-3 rounded-[28px] border border-dashed border-border/70 px-5 py-8 text-center transition hover:border-[color:var(--brand-teal)] hover:bg-[color:color-mix(in_oklab,var(--brand-teal)_5%,var(--surface-soft))] disabled:pointer-events-none disabled:opacity-60",
        className
      )}
      {...props}
    >
      {preview ? (
        <div className="w-full overflow-hidden rounded-[20px]">{preview}</div>
      ) : (
        <div className="flex size-14 items-center justify-center rounded-2xl bg-[var(--surface-1)] text-muted-foreground shadow-[0_18px_30px_-24px_color-mix(in_oklab,var(--foreground)_35%,transparent)]">
          <UploadCloud className="size-6" />
        </div>
      )}
      <div className="space-y-1">
        <p className="font-medium">{title}</p>
        <p className="text-sm text-muted-foreground">{description}</p>
      </div>
      <div className="flex items-center gap-2">
        <Button variant="pill" size="pill" asChild>
          <span>{actionLabel}</span>
        </Button>
        {children}
      </div>
    </button>
  )
}

type StageEmptyStateProps = ComponentProps<"div"> & {
  title: string
  description: string
  icon?: ComponentType<{ className?: string }>
  action?: ReactNode
}

export function StageEmptyState({
  title,
  description,
  icon: Icon,
  action,
  className,
  ...props
}: StageEmptyStateProps) {
  return (
    <div
      className={cn(
        "workspace-surface workspace-halo flex min-h-[360px] flex-col items-center justify-center rounded-[32px] px-6 py-12 text-center",
        className
      )}
      {...props}
    >
      {Icon ? (
        <div className="mb-5 flex size-16 items-center justify-center rounded-[24px] bg-[var(--surface-soft)] text-muted-foreground">
          <Icon className="size-7" />
        </div>
      ) : null}
      <h3 className="text-xl font-semibold tracking-tight">{title}</h3>
      <p className="mt-2 max-w-md text-sm leading-6 text-muted-foreground">{description}</p>
      {action ? <div className="mt-5">{action}</div> : null}
    </div>
  )
}

type CompactConfigSummaryProps = ComponentProps<"div"> & {
  title?: string
  items: Array<{
    label: string
    value: ReactNode
  }>
}

export function CompactConfigSummary({
  title = "Summary",
  items,
  className,
  ...props
}: CompactConfigSummaryProps) {
  return (
    <SoftPanel className={cn("space-y-4", className)} {...props}>
      <div className="flex items-center justify-between gap-3">
        <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
          {title}
        </p>
        <Badge variant="chip">{items.length} items</Badge>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        {items.map((item) => (
          <div key={item.label} className="space-y-1 rounded-2xl bg-[var(--surface-1)] px-4 py-3">
            <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
              {item.label}
            </p>
            <p className="text-sm font-medium text-foreground">{item.value}</p>
          </div>
        ))}
      </div>
    </SoftPanel>
  )
}

export function WorkspaceStage({
  className,
  ...props
}: ComponentProps<"section">) {
  return (
    <section
      className={cn(
        "workspace-surface flex min-h-0 flex-1 flex-col overflow-hidden rounded-[32px]",
        className
      )}
      {...props}
    />
  )
}
