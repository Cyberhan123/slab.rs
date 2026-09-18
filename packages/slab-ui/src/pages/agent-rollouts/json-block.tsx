import { cn } from "@slab/ui/lib/utils"

/**
 * Compact read-only JSON viewer for rollout/trace payloads.
 *
 * `wrap-anywhere` (not just `break-words`): payloads carry unbroken strings
 * (base64 images, paths) whose intrinsic min-content width would otherwise
 * keep the `<pre>` wider than the pane, forcing a horizontal scrollbar
 * instead of wrapping.
 */
export function JsonBlock({
  value,
  className,
}: {
  value: unknown
  className?: string
}) {
  return (
    <pre
      className={cn(
        "max-h-full overflow-auto rounded-lg bg-muted/40 p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap wrap-anywhere",
        className,
      )}
    >
      {formatJson(value)}
    </pre>
  )
}

function formatJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2) ?? "null"
  } catch {
    return String(value)
  }
}
