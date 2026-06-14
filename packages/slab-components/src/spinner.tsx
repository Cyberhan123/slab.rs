import { Loader2Icon } from "lucide-react"

import { cn } from "./lib/utils"

function Spinner({
  className,
  role = "status",
  "aria-label": ariaLabel = "Loading",
  ...props
}: React.ComponentProps<"svg">) {
  return (
    <Loader2Icon
      aria-label={ariaLabel}
      className={cn("size-4 animate-spin", className)}
      role={role}
      {...props}
    />
  )
}

export { Spinner }
