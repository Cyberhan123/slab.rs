import {
  ImagePlus,
  Mic,
  Plus,
  Search,
  SendHorizontal,
  Square,
  WandSparkles,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Textarea } from "@/components/ui/textarea"
import { cn } from "@/lib/utils"

type ChatComposerProps = {
  value: string
  onValueChange: (value: string) => void
  onSubmit: (value: string) => void | Promise<void>
  onCancel: () => void
  isRequesting: boolean
  deepThink: boolean
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
  deepThink,
  setDeepThink,
  onGenerateImage,
  statusLabel,
}: ChatComposerProps) {
  const handleSubmit = () => {
    if (!value.trim() || isRequesting) {
      return
    }

    void onSubmit(value.trim())
  }

  return (
    <div className="space-y-3">
      <div className="rounded-[24px] bg-[#e0e3e5] p-[5px] shadow-[0_1px_2px_rgba(0,0,0,0.05)]">
        <div className="flex items-end gap-2 px-4 py-2">
          <div className="pb-1">
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="quiet"
                  size="icon"
                  className="size-10 rounded-full border border-white/50 bg-white/60 text-[#6d7a77] hover:bg-white"
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
            onChange={(event) => onValueChange(event.target.value)}
            placeholder="Type a message or drop files..."
            className="min-h-[48px] max-h-48 resize-none border-0 bg-transparent px-3 py-3 text-base text-[#191c1e] shadow-none placeholder:text-[#6d7a77]/60 focus-visible:ring-0"
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
              className="size-10 rounded-full text-[#6d7a77] hover:bg-white/70 hover:text-[#3d4947]"
              disabled
            >
              <Mic className="size-4" />
            </Button>

            <Button
              variant="cta"
              size="icon"
              className={cn(
                "size-10 rounded-full shadow-[0_10px_15px_-3px_rgba(0,104,95,0.2),0_4px_6px_-4px_rgba(0,104,95,0.2)]",
                isRequesting && "bg-[#191c1e] text-white shadow-none"
              )}
              onClick={() => {
                if (isRequesting) {
                  onCancel()
                  return
                }

                handleSubmit()
              }}
              disabled={!isRequesting && !value.trim()}
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
            className="inline-flex items-center gap-1.5 text-[11px] font-bold text-[#6d7a77] transition disabled:cursor-not-allowed disabled:opacity-100"
          >
            <Search className="size-3" />
            Web Search
          </button>

          <button
            type="button"
            aria-pressed={deepThink}
            onClick={() => setDeepThink(!deepThink)}
            className={cn(
              "inline-flex items-center gap-1.5 text-[11px] font-bold transition",
              deepThink ? "text-[#191c1e]" : "text-[#6d7a77] hover:text-[#191c1e]"
            )}
          >
            <WandSparkles className={cn("size-3", deepThink && "text-[var(--brand-teal)]")} />
            {deepThink ? "Deep Think On" : "Deep Think"}
          </button>

          <button
            type="button"
            onClick={onGenerateImage}
            className="inline-flex items-center gap-1.5 text-[11px] font-bold text-[#6d7a77] transition hover:text-[#191c1e]"
          >
            <ImagePlus className="size-3" />
            Generate Image
          </button>
        </div>

        <p className="max-w-full text-[10px] font-medium text-[#6d7a77]/70">{statusLabel}</p>
      </div>
    </div>
  )
}
