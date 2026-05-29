import {
  FileText,
  ImagePlus,
  Mic,
  Network,
  Plus,
  Search,
  WandSparkles,
  Wrench,
} from "lucide-react"
import { type ReactNode, useCallback, useMemo } from "react"
import { Sender, Suggestion, type SuggestionProps } from "@ant-design/x"

import { Button } from "@slab/components/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@slab/components/dropdown-menu"
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

function renderSenderSuffix(originNode: ReactNode) {
  return (
    <div className="flex items-end gap-2">
      <Button
        variant="quiet"
        size="icon"
        className="size-10 rounded-full text-muted-foreground hover:bg-[var(--shell-card)]/45 hover:text-foreground"
        disabled
      >
        <Mic className="size-4" />
      </Button>
      {originNode}
    </div>
  )
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
  const matchCommandItems = useCallback(
    (query: string) =>
      commandItems.filter((item) => {
        const normalizedCommand = item.command.slice(1).toLowerCase()
        const normalizedLabel = item.label.toLowerCase()

        return normalizedCommand.startsWith(query) || normalizedLabel.startsWith(query)
      }),
    [commandItems]
  )
  const commandQuery = value.match(/^\/([^\s/]*)$/)?.[1]?.toLowerCase() ?? null
  const matchingCommandItems = useMemo(
    () => (commandQuery === null ? [] : matchCommandItems(commandQuery)),
    [commandQuery, matchCommandItems]
  )
  const commandSuggestions = useMemo<SuggestionProps<{ query: string }>["items"]>(
    () => (info) =>
      matchCommandItems(info?.query ?? "").map((item) => {
        const Icon = item.icon

        return {
          icon: <Icon className="size-4" />,
          label: (
            <span className="min-w-0">
              <span className="flex items-center gap-2 text-sm font-semibold">
                <span className="font-mono text-[13px]">{item.command}</span>
                <span>{item.label}</span>
              </span>
              <span className="block truncate text-[11px] opacity-70">{item.description}</span>
            </span>
          ),
          value: item.command,
        }
      }),
    [matchCommandItems]
  )
  const webSearchActive = value.trimStart().startsWith("/web_search")

  const insertCommand = (command: string) => {
    onValueChange(`${command} `)
  }

  const handleSubmit = (nextValue: string) => {
    const prompt = nextValue.trim()
    if (!prompt || isRequesting || disabled) {
      return
    }

    void onSubmit(prompt)
  }

  return (
    <div className="relative space-y-3">
      <Suggestion<{ query: string }>
        block
        items={commandSuggestions}
        onSelect={(command) => insertCommand(command)}
      >
        {({ onTrigger, onKeyDown, open }) => (
          <Sender
            value={value}
            loading={isRequesting}
            disabled={disabled}
            submitType="enter"
            onCancel={onCancel}
            onChange={(nextValue) => {
              onValueChange(nextValue)
              const nextQuery = nextValue.match(/^\/([^\s/]*)$/)?.[1]?.toLowerCase()
              if (nextQuery === undefined || disabled || matchCommandItems(nextQuery).length === 0) {
                onTrigger(false)
                return
              }

              onTrigger({ query: nextQuery })
            }}
            onSubmit={(message) => handleSubmit(message)}
            onKeyDown={(event) => {
              if (open && event.key === "Enter" && !event.shiftKey && matchingCommandItems[0]) {
                event.preventDefault()
                insertCommand(matchingCommandItems[0].command)
                onTrigger(false)
                return false
              }

              return onKeyDown(event)
            }}
            placeholder={t("pages.assistant.composer.placeholder")}
            autoSize={{ minRows: 2, maxRows: 6 }}
            className="rounded-[24px] border-0 bg-[var(--surface-input)] p-[5px] shadow-[var(--shell-elevation)]"
            classNames={{
              content: "flex items-end gap-2 px-4 py-2",
              input:
                "min-h-[48px] max-h-48 resize-none bg-transparent px-3 py-3 text-base text-foreground placeholder:text-muted-foreground/60",
              prefix: "pb-1",
              suffix: "pb-1",
            }}
            prefix={
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
            }
            suffix={renderSenderSuffix}
          />
        )}
      </Suggestion>

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
