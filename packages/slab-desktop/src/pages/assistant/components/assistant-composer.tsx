import {
  ChevronDown,
  FileText,
  ImagePlus,
  Mic,
  Network,
  Plus,
  Search,
  SlidersHorizontal,
  WandSparkles,
  Wrench,
} from "lucide-react"
import { useCallback, useMemo } from "react"
import { Sender, Suggestion, type SuggestionProps } from "@ant-design/x"
import type { ActionsComponents } from "@ant-design/x/es/sender"

import { Button } from "@slab/components/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@slab/components/dropdown-menu"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@slab/components/select"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@slab/components/collapsible"
import { Textarea } from "@slab/components/textarea"
import { useTranslation } from "@slab/i18n"
import { cn } from "@/lib/utils"
import type {
  AssistantReasoningEffort,
  AssistantToolChoice,
} from "@/store/useAssistantUiStore"

type AssistantComposerProps = {
  value: string
  onValueChange: (value: string) => void
  onSubmit: (value: string) => void | Promise<void>
  onCancel: () => void
  isRequesting: boolean
  disabled?: boolean
  reasoningEffort: AssistantReasoningEffort
  reasoningSupported: boolean
  setReasoningEffort: (value: AssistantReasoningEffort) => void
  systemPrompt: string
  setSystemPrompt: (value: string) => void
  toolConcurrency: number
  setToolConcurrency: (value: number) => void
  toolChoice: AssistantToolChoice
  setToolChoice: (value: AssistantToolChoice) => void
  advancedPanelOpen: boolean
  setAdvancedPanelOpen: (value: boolean) => void
  onGenerateImage: () => void
  statusLabel: string
}

function renderSenderSuffix(
  components: ActionsComponents,
  labels: { cancel: string; send: string; voice: string },
  isRequesting: boolean
) {
  const { LoadingButton, SendButton } = components

  return (
    <div className="flex items-end gap-2">
      <Button
        aria-label={labels.voice}
        variant="quiet"
        size="icon"
        className="size-10 rounded-full text-muted-foreground hover:bg-glass-bg hover:text-foreground"
        disabled
      >
        <Mic className="size-4" />
      </Button>
      {isRequesting ? (
        <LoadingButton aria-label={labels.cancel} />
      ) : (
        <span data-testid="assistant-send-button">
          <SendButton aria-label={labels.send} />
        </span>
      )}
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
  reasoningEffort,
  reasoningSupported,
  setReasoningEffort,
  systemPrompt,
  setSystemPrompt,
  toolConcurrency,
  setToolConcurrency,
  toolChoice,
  setToolChoice,
  advancedPanelOpen,
  setAdvancedPanelOpen,
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
                <span className="font-mono text-body">{item.command}</span>
                <span>{item.label}</span>
              </span>
              <span className="block truncate text-caption opacity-70">{item.description}</span>
            </span>
          ),
          value: item.command,
        }
      }),
    [matchCommandItems]
  )
  const webSearchActive = value.trimStart().startsWith("/web_search")
  const reasoningActive = reasoningSupported && reasoningEffort !== "none"
  const resolvedToolChoice = toolChoice ?? { type: "auto" }

  const insertCommand = (command: string) => {
    onValueChange(`${command} `)
  }

  const setToolChoiceType = (nextType: "auto" | "none" | "required") => {
    setToolChoice({ type: nextType })
  }

  const handleSubmit = (nextValue: string) => {
    const prompt = nextValue.trim()
    if (!prompt || isRequesting || disabled) {
      return
    }

    void onSubmit(prompt)
  }

  return (
    <div className="relative space-y-3" data-testid="assistant-composer">
      <Suggestion<{ query: string }>
        block
        items={commandSuggestions}
        onSelect={(command) => insertCommand(command)}
      >
        {({ onTrigger, onKeyDown, open }) => (
          <div data-testid="assistant-composer-input">
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
              className="rounded-2xl border-0 bg-[var(--surface-input)] p-[5px] shadow-[var(--shell-elevation)]"
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
                      className="size-10 rounded-full border border-transparent bg-transparent text-muted-foreground hover:bg-glass-bg hover:text-foreground"
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
              suffix={(_, { components }) =>
                renderSenderSuffix(
                  components,
                  {
                    cancel: t("pages.assistant.composer.cancel"),
                    send: t("pages.assistant.composer.sendMessage"),
                    voice: t("pages.assistant.composer.voiceCapture"),
                  },
                  isRequesting
                )
              }
            />
          </div>
        )}
      </Suggestion>

      <Collapsible open={advancedPanelOpen} onOpenChange={setAdvancedPanelOpen}>
        <div className="flex flex-wrap items-center justify-between gap-3 px-2">
        <div className="flex flex-wrap items-center gap-4">
          <button
            type="button"
            disabled={disabled}
            aria-pressed={webSearchActive}
            data-testid="assistant-web-search-toggle"
            onClick={() => insertCommand("/web_search")}
            className={cn(
              "inline-flex items-center gap-1.5 text-caption font-bold transition",
              webSearchActive
                ? "text-foreground"
                : "text-muted-foreground hover:text-foreground",
              disabled && "cursor-not-allowed opacity-60"
            )}
          >
            <Search className={cn("size-3", webSearchActive && "text-[color:var(--brand-teal)]")} />
            {t("pages.assistant.composer.webSearch")}
          </button>

          <button
            type="button"
            disabled={disabled || !reasoningSupported}
            aria-pressed={reasoningActive}
            data-testid="assistant-reasoning-toggle"
            onClick={() => setReasoningEffort(reasoningActive ? "none" : "medium")}
            className={cn(
              "inline-flex items-center gap-1.5 text-caption font-bold transition",
              reasoningActive
                ? "text-foreground"
                : "text-muted-foreground hover:text-foreground",
              disabled && "cursor-not-allowed opacity-60"
            )}
          >
            <WandSparkles
              className={cn(
                "size-3",
                reasoningActive && "text-[color:var(--brand-teal)]"
              )}
            />
            {!reasoningSupported
              ? t("pages.assistant.composer.deepThinkUnavailable")
              : reasoningActive
                ? t("pages.assistant.composer.reasoningActive", {
                  effort: t(`pages.assistant.composer.reasoning.${reasoningEffort}`),
                })
                : t("pages.assistant.composer.reasoningOff")}
          </button>

          <button
            type="button"
            disabled={disabled}
            data-testid="assistant-generate-image-button"
            onClick={onGenerateImage}
            className={cn(
              "inline-flex items-center gap-1.5 text-caption font-bold text-muted-foreground transition hover:text-foreground",
              disabled && "cursor-not-allowed opacity-60"
            )}
          >
            <ImagePlus className="size-3" />
            {t("pages.assistant.composer.generateImage")}
          </button>

          <CollapsibleTrigger asChild>
            <button
              type="button"
              disabled={disabled}
              data-testid="assistant-advanced-toggle"
              className={cn(
                "inline-flex items-center gap-1.5 text-caption font-bold text-muted-foreground transition hover:text-foreground",
                advancedPanelOpen && "text-foreground",
                disabled && "cursor-not-allowed opacity-60"
              )}
            >
              <SlidersHorizontal className="size-3" />
              {t("pages.assistant.composer.advanced")}
              <ChevronDown
                className={cn(
                  "size-3 transition-transform",
                  advancedPanelOpen && "rotate-180"
                )}
              />
            </button>
          </CollapsibleTrigger>
        </div>

        <p className="max-w-full text-micro font-medium text-muted-foreground/70">{statusLabel}</p>
      </div>

        <CollapsibleContent className="px-2 pt-3">
          <div
            className="grid gap-3 rounded-[20px] border border-border/60 bg-[var(--surface-soft)] p-3 shadow-[inset_0_1px_0_color-mix(in_oklab,var(--foreground)_4%,transparent)] md:grid-cols-3"
            data-testid="assistant-advanced-panel"
          >
            <label className="grid gap-1.5 text-caption font-semibold text-muted-foreground">
              <span>{t("pages.assistant.composer.reasoningEffort")}</span>
              <Select
                value={reasoningEffort}
                disabled={disabled || !reasoningSupported}
                onValueChange={(nextValue) =>
                  setReasoningEffort(nextValue as AssistantReasoningEffort)
                }
              >
                <SelectTrigger variant="soft" className="h-9 w-full text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent variant="soft">
                  {(["none", "minimal", "low", "medium", "high"] as const).map((item) => (
                    <SelectItem key={item} value={item}>
                      {t(`pages.assistant.composer.reasoning.${item}`)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>

            <label className="grid gap-1.5 text-caption font-semibold text-muted-foreground">
              <span>{t("pages.assistant.composer.toolChoice")}</span>
              <Select
                value={resolvedToolChoice.type}
                disabled={disabled}
                onValueChange={(nextValue) =>
                  setToolChoiceType(nextValue as "auto" | "none" | "required")
                }
              >
                <SelectTrigger variant="soft" className="h-9 w-full text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent variant="soft">
                  {(["auto", "none", "required"] as const).map((item) => (
                    <SelectItem key={item} value={item}>
                      {t(`pages.assistant.composer.toolChoiceOptions.${item}`)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>

            <label className="grid gap-1.5 text-caption font-semibold text-muted-foreground">
              <span>{t("pages.assistant.composer.toolConcurrency")}</span>
              <Select
                value={String(toolConcurrency)}
                disabled={disabled}
                onValueChange={(nextValue) => setToolConcurrency(Number(nextValue))}
              >
                <SelectTrigger variant="soft" className="h-9 w-full text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent variant="soft">
                  {[1, 2, 3, 4].map((item) => (
                    <SelectItem key={item} value={String(item)}>
                      {item}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </label>

            <label className="grid gap-1.5 text-caption font-semibold text-muted-foreground md:col-span-3">
              <span>{t("pages.assistant.composer.systemPrompt")}</span>
              <Textarea
                value={systemPrompt}
                disabled={disabled}
                onChange={(event) => setSystemPrompt(event.currentTarget.value)}
                className="min-h-20 resize-y text-xs leading-5"
                placeholder={t("pages.assistant.composer.systemPromptPlaceholder")}
              />
            </label>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  )
}
