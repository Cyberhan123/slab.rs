import { Loader2Icon } from "lucide-react"

import { cn } from "@/lib/utils"

function Spinner({ className, ...props }: React.ComponentProps<"svg">) {
  return (
    <Loader2Icon
      // Upstream: role=status announces loading; an <output> element would
      // change the SVG's rendering context.
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- vendored upstream shape
      role="status"
      aria-label="Loading"
      className={cn("size-4 animate-spin", className)}
      {...props}
    />
  )
}

export { Spinner }
