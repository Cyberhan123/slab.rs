import {
  FileText,
  ImagePlus,
  Mic,
  Network,
  Plus,
  Search,
  SendHorizontal,
  Square,
  WandSparkles,
  Wrench,
} from "lucide-react"
import { useMemo } from "react"

import { Button } from "@slab/components/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@slab/components/dropdown-menu"
import { Textarea } from "@slab/components/textarea"
import { useTranslation } from "@slab/i18n"
import { cn } from "@/lib/utils"

type AssistantComposerProps = {
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

export function AssistantComposer({
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
}: AssistantComposerProps) {
  const { t } = useTranslation()
  const commandItems = useMemo(
    () => [
      {
        command: "/plan",
        description: t("pages.assistant.composer.commandPlanDescription"),
        icon: FileText,
        label: t("pages.assistant.composer.commandPlan"),
      },
      {
        command: "/skill",
        description: t("pages.assistant.composer.commandSkillDescription"),
        icon: Wrench,
        label: t("pages.assistant.composer.commandSkill"),
      },
      {
        command: "/mcp",
        description: t("pages.assistant.composer.commandMcpDescription"),
        icon: Network,
        label: t("pages.assistant.composer.commandMcp"),
      },
      {
        command: "/web_search",
        description: t("pages.assistant.composer.commandWebSearchDescription"),
        icon: Search,
        label: t("pages.assistant.composer.commandWebSearch"),
      },
    ],
    [t]
  )
  const commandQuery = value.match(/^\/([^\s/]*)$/)?.[1]?.toLowerCase() ?? null
  const matchingCommandItems = useMemo(() => {
    if (commandQuery === null) {
      return []
    }

    return commandItems.filter((item) => {
      const normalizedCommand = item.command.slice(1).toLowerCase()
      const normalizedLabel = item.label.toLowerCase()

      return (
        normalizedCommand.startsWith(commandQuery) ||
        normalizedLabel.startsWith(commandQuery)
      )
    })
  }, [commandItems, commandQuery])
  const showCommandMenu = !disabled && commandQuery !== null && matchingCommandItems.length > 0
  const webSearchActive = value.trimStart().startsWith("/web_search")

  const insertCommand = (command: string) => {
    onValueChange(`${command} `)
  }

  const handleSubmit = () => {
    if (!value.trim() || isRequesting || disabled) {
      return
    }

    void onSubmit(value.trim())
  }

  return (
    <div className="relative space-y-3">
      {showCommandMenu ? (
        <div className="absolute bottom-[calc(100%+12px)] left-2 z-30 w-[min(24rem,calc(100vw-3rem))] overflow-hidden rounded-[18px] border border-border/70 bg-[var(--surface-1)] p-1.5 shadow-[0_22px_50px_-34px_color-mix(in_oklab,var(--foreground)_40%,transparent)]">
          {matchingCommandItems.map((item) => {
            const Icon = item.icon

            return (
              <button
                key={item.command}
                type="button"
                className="flex w-full items-center gap-3 rounded-[12px] px-3 py-2.5 text-left transition hover:bg-[var(--surface-soft)]"
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => insertCommand(item.command)}
              >
                <span className="flex size-8 shrink-0 items-center justify-center rounded-[8px] bg-[var(--brand-teal)]/12 text-[var(--brand-teal)]">
                  <Icon className="size-4" />
                </span>
                <span className="min-w-0">
                  <span className="flex items-center gap-2 text-sm font-semibold text-foreground">
                    <span className="font-mono text-[13px]">{item.command}</span>
                    <span>{item.label}</span>
                  </span>
                  <span className="block truncate text-[11px] text-muted-foreground">
                    {item.description}
                  </span>
                </span>
              </button>
            )
          })}
        </div>
      ) : null}
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
                {commandItems.map((item) => {
                  const Icon = item.icon

                  return (
                    <DropdownMenuItem
                      key={item.command}
                      onClick={() => insertCommand(item.command)}
                    >
                      <Icon className="size-4" />
                      <span className="font-mono text-xs">{item.command}</span>
                      <span>{item.label}</span>
                    </DropdownMenuItem>
                  )
                })}
                <DropdownMenuItem onClick={onGenerateImage}>
                  <ImagePlus className="size-4" />
                  {t("pages.assistant.composer.generateImage")}
                </DropdownMenuItem>
                <DropdownMenuItem disabled>
                  <Mic className="size-4" />
                  {t("pages.assistant.composer.voiceCapture")}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>

          <Textarea
            value={value}
            variant="shell"
            autoResize
            disabled={disabled}
            onChange={(event) => onValueChange(event.target.value)}
            placeholder={t("pages.assistant.composer.placeholder")}
            className="min-h-[48px] max-h-48 resize-none border-0 bg-transparent px-3 py-3 text-base text-foreground shadow-none placeholder:text-muted-foreground/60 focus-visible:ring-0"
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault()
                if (showCommandMenu && matchingCommandItems[0]) {
                  insertCommand(matchingCommandItems[0].command)
                  return
                }

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
              aria-label={
                isRequesting
                  ? t("pages.assistant.composer.stopGeneratingResponse")
                  : t("pages.assistant.composer.sendMessage")
              }
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
            disabled={disabled}
            aria-pressed={webSearchActive}
            onClick={() => insertCommand("/web_search")}
            className={cn(
              "inline-flex items-center gap-1.5 text-[11px] font-bold transition",
              webSearchActive
                ? "text-foreground"
                : "text-muted-foreground hover:text-foreground",
              disabled && "cursor-not-allowed opacity-60"
            )}
          >
            <Search className={cn("size-3", webSearchActive && "text-[var(--brand-teal)]")} />
            {t("pages.assistant.composer.webSearch")}
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
            {!reasoningSupported
              ? t("pages.assistant.composer.deepThinkUnavailable")
              : deepThink
                ? t("pages.assistant.composer.deepThinkOn")
                : t("pages.assistant.composer.deepThink")}
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
            {t("pages.assistant.composer.generateImage")}
          </button>
        </div>

        <p className="max-w-full text-[10px] font-medium text-muted-foreground/70">{statusLabel}</p>
      </div>
    </div>
  )
}
