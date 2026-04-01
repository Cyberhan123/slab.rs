import {
  ImagePlus,
  Mic,
  Plus,
  Search,
  SendHorizontal,
  Square,
  WandSparkles,
} from "lucide-react"

import { Button } from "@slab/components/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@slab/components/dropdown-menu"
import { Textarea } from "@slab/components/textarea"
import { cn } from "@/lib/utils"

type ChatComposerProps = {
  value: string
  onValueChange: (value: string) => void
  onSubmit: (value: string) => void | Promise<void>
  onCancel: () => void
  isRequesting: boolean
  disabled?: boolean
  deepThink: boolean
  reasoningSupported: boolean
  setDeepThink: (value: boolean) => void
  onGenerateImage: () => void
  statusLabel: string
}

export function ChatComposer({
  value,
  onValueChange,
  onSubmit,
  onCancel,
  isRequesting,
  disabled = false,
  deepThink,
  reasoningSupported,
  setDeepThink,
  onGenerateImage,
  statusLabel,
}: ChatComposerProps) {
  const handleSubmit = () => {
    if (!value.trim() || isRequesting || disabled) {
      return
    }

    void onSubmit(value.trim())
  }

  return (
    <div className="space-y-3">
      <div className="rounded-[24px] bg-[var(--surface-input)] p-[5px] shadow-[var(--shell-elevation)]">
        <div className="flex items-end gap-2 px-4 py-2">
          <div className="pb-1">
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="quiet"
                  size="icon"
                  disabled={disabled}
                  className="size-10 rounded-full border border-transparent bg-transparent text-muted-foreground hover:bg-[var(--shell-card)]/45 hover:text-foreground"
                >
                  <Plus className="size-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start" className="rounded-2xl border-border/70">
                <DropdownMenuItem onClick={onGenerateImage}>
                  <ImagePlus className="size-4" />
                  Generate image
                </DropdownMenuItem>
                <DropdownMenuItem disabled>
                  <Search className="size-4" />
                  Web search
                </DropdownMenuItem>
                <DropdownMenuItem disabled>
                  <Mic className="size-4" />
                  Voice capture
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>

          <Textarea
            value={value}
            variant="shell"
            disabled={disabled}
            onChange={(event) => onValueChange(event.target.value)}
            placeholder="Type a message or drop files..."
            className="min-h-[48px] max-h-48 resize-none border-0 bg-transparent px-3 py-3 text-base text-foreground shadow-none placeholder:text-muted-foreground/60 focus-visible:ring-0"
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault()
                handleSubmit()
              }
            }}
          />

          <div className="flex items-end gap-2 pb-1">
            <Button
              variant="quiet"
              size="icon"
              className="size-10 rounded-full text-muted-foreground hover:bg-[var(--shell-card)]/45 hover:text-foreground"
              disabled
            >
              <Mic className="size-4" />
            </Button>

            <Button
              variant="cta"
              size="icon"
              className={cn(
                "size-10 rounded-full shadow-[0_10px_15px_-3px_color-mix(in oklab,var(--brand-teal) 20%,transparent),0_4px_6px_-4px_color-mix(in oklab,var(--brand-teal) 20%,transparent)]",
                isRequesting && "bg-foreground text-background shadow-none"
              )}
              onClick={() => {
                if (disabled) {
                  return
                }

                if (isRequesting) {
                  onCancel()
                  return
                }

                handleSubmit()
              }}
              disabled={disabled || (!isRequesting && !value.trim())}
              aria-label={isRequesting ? "Stop generating response" : "Send message"}
            >
              {isRequesting ? <Square className="size-4" /> : <SendHorizontal className="size-4" />}
            </Button>
          </div>
        </div>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3 px-2">
        <div className="flex flex-wrap items-center gap-4">
          <button
            type="button"
            disabled
            className="inline-flex items-center gap-1.5 text-[11px] font-bold text-muted-foreground transition disabled:cursor-not-allowed disabled:opacity-100"
          >
            <Search className="size-3" />
            Web Search
          </button>

          <button
            type="button"
            disabled={disabled || !reasoningSupported}
            aria-pressed={deepThink}
            onClick={() => setDeepThink(!deepThink)}
            className={cn(
              "inline-flex items-center gap-1.5 text-[11px] font-bold transition",
              reasoningSupported && deepThink
                ? "text-foreground"
                : "text-muted-foreground hover:text-foreground",
              disabled && "cursor-not-allowed opacity-60"
            )}
          >
            <WandSparkles
              className={cn(
                "size-3",
                reasoningSupported && deepThink && "text-[var(--brand-teal)]"
              )}
            />
            {!reasoningSupported ? "Deep Think Unavailable" : deepThink ? "Deep Think On" : "Deep Think"}
          </button>

          <button
            type="button"
            disabled={disabled}
            onClick={onGenerateImage}
            className={cn(
              "inline-flex items-center gap-1.5 text-[11px] font-bold text-muted-foreground transition hover:text-foreground",
              disabled && "cursor-not-allowed opacity-60"
            )}
          >
            <ImagePlus className="size-3" />
            Generate Image
          </button>
        </div>

        <p className="max-w-full text-[10px] font-medium text-muted-foreground/70">{statusLabel}</p>
      </div>
    </div>
  )
}
