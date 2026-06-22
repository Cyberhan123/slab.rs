import { cn } from "./lib/utils"

function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      className={cn("skeleton-shimmer rounded-md", className)}
      {...props}
    />
  )
}

function SkeletonText({
  className,
  lines = 3,
  ...props
}: React.ComponentProps<"div"> & {
  lines?: number
}) {
  const lineItems = Array.from({ length: Math.max(0, lines) }, (_, index) => ({
    className: index === lines - 1 ? "w-2/3" : "w-full",
    key: `skeleton-text-line-${index + 1}`,
  }))

  return (
    <div
      data-slot="skeleton-text"
      className={cn("flex flex-col gap-2", className)}
      {...props}
    >
      {lineItems.map((line) => (
        <Skeleton key={line.key} className={cn("h-3", line.className)} />
      ))}
    </div>
  )
}

function SkeletonCircle({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton-circle"
      className={cn("skeleton-shimmer size-10 rounded-full", className)}
      {...props}
    />
  )
}

export { Skeleton, SkeletonText, SkeletonCircle }
