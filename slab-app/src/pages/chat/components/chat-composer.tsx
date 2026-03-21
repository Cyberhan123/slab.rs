import { ImagePlus, Loader2, Mic, Plus, Search, SendHorizontal, Square, WandSparkles } from "lucide-react"
import { useMemo } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { cn } from "@/lib/utils"

type ModelOption = {
  id: string
  label: string
  downloaded: boolean
  pending: boolean
  source: "local" | "cloud"
}

type ChatComposerProps = {
  value: string
  onValueChange: (value: string) => void
  onSubmit: (value: string) => void | Promise<void>
  onCancel: () => void
  isRequesting: boolean
  deepThink: boolean
  setDeepThink: (value: boolean) => void
  modelOptions: ModelOption[]
  selectedModelId: string
  onModelChange: (id: string) => void
  modelLoading?: boolean
  modelDisabled?: boolean
  onGenerateImage: () => void
}

export function ChatComposer({
  value,
  onValueChange,
  onSubmit,
  onCancel,
  isRequesting,
  deepThink,
  setDeepThink,
  modelOptions,
  selectedModelId,
  onModelChange,
  modelLoading = false,
  modelDisabled = false,
  onGenerateImage,
}: ChatComposerProps) {
  const selectedModel = useMemo(
    () => modelOptions.find((option) => option.id === selectedModelId),
    [modelOptions, selectedModelId]
  )

  const handleSubmit = () => {
    if (!value.trim() || isRequesting) {
      return
    }

    void onSubmit(value.trim())
  }

  return (
    <div className="workspace-surface rounded-[32px] p-3 shadow-[0_26px_60px_-36px_color-mix(in_oklab,var(--foreground)_35%,transparent)]">
      <div className="flex gap-3">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="pill" size="icon" className="rounded-full">
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

        <div className="flex-1 space-y-3">
          <Textarea
            value={value}
            variant="shell"
            onChange={(event) => onValueChange(event.target.value)}
            placeholder="Ask for analysis, debugging help, or a polished draft..."
            className="min-h-[120px] resize-none border-none"
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault()
                handleSubmit()
              }
            }}
          />

          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex flex-wrap items-center gap-2">
              <div className="workspace-soft-panel flex items-center gap-2 rounded-full px-3 py-2">
                <WandSparkles className="size-4 text-muted-foreground" />
                <span className="text-sm font-medium">Deep think</span>
                <Switch
                  checked={deepThink}
                  onCheckedChange={setDeepThink}
                  variant="workspace"
                />
              </div>

              <Popover>
                <PopoverTrigger asChild>
                  <Button variant="pill" size="pill" disabled={modelDisabled || modelLoading}>
                    {modelLoading ? (
                      <>
                        <Loader2 className="size-4 animate-spin" />
                        Loading models
                      </>
                    ) : (
                      <>
                        <Badge variant="chip">{selectedModel?.source ?? "model"}</Badge>
                        <span className="max-w-[180px] truncate">
                          {selectedModel?.label ?? "Select model"}
                        </span>
                      </>
                    )}
                  </Button>
                </PopoverTrigger>
                <PopoverContent className="w-80 rounded-[24px] border-border/70 bg-[var(--surface-1)]">
                  <div className="space-y-3">
                    <div>
                      <p className="text-sm font-semibold">Chat model</p>
                      <p className="text-sm text-muted-foreground">
                        Local models auto-prepare before the request. Cloud models switch instantly.
                      </p>
                    </div>
                    <Select
                      value={selectedModelId}
                      onValueChange={onModelChange}
                      disabled={modelDisabled || modelLoading}
                    >
                      <SelectTrigger variant="soft" className="w-full">
                        <SelectValue placeholder="Select model" />
                      </SelectTrigger>
                      <SelectContent variant="soft">
                        {modelOptions.length === 0 ? (
                          <SelectItem value="__none" disabled>
                            No chat models available
                          </SelectItem>
                        ) : (
                          modelOptions.map((option) => (
                            <SelectItem key={option.id} value={option.id}>
                              <div className="flex min-w-0 items-center gap-2">
                                <span className="truncate">{option.label}</span>
                                {option.pending ? <Badge variant="chip">Downloading</Badge> : null}
                                {!option.downloaded && option.source === "local" ? (
                                  <Badge variant="chip">Not downloaded</Badge>
                                ) : null}
                              </div>
                            </SelectItem>
                          ))
                        )}
                      </SelectContent>
                    </Select>
                  </div>
                </PopoverContent>
              </Popover>

              <Button variant="quiet" size="icon" className="rounded-full" disabled>
                <Mic className="size-4" />
              </Button>
            </div>

            <Button
              variant={isRequesting ? "pill" : "cta"}
              size="pill"
              className={cn("min-w-[120px]", isRequesting && "border-border/60")}
              onClick={() => {
                if (isRequesting) {
                  onCancel()
                  return
                }

                handleSubmit()
              }}
              disabled={!isRequesting && !value.trim()}
            >
              {isRequesting ? (
                <>
                  <Square className="size-4" />
                  Stop
                </>
              ) : (
                <>
                  <SendHorizontal className="size-4" />
                  Send
                </>
              )}
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
