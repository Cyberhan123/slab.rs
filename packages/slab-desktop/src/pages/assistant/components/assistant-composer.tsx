import {
  ChevronDown,
  FileText,
  ImagePlus,
  Mic,
  Network,
  Plus,
  Search,
  Send,
  SlidersHorizontal,
  Square,
  WandSparkles,
  Wrench,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { Button } from '@slab/components/button'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@slab/components/collapsible'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandList,
} from '@slab/components/command'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@slab/components/dropdown-menu'
import {
  Field,
  FieldGroup,
  FieldLabel,
} from '@slab/components/field'
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from '@slab/components/popover'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@slab/components/select'
import { Textarea } from '@slab/components/textarea'
import { useTranslation } from '@slab/i18n'
import { cn } from '@/lib/utils'
import type {
  AssistantReasoningEffort,
  AssistantToolChoice,
} from '@/store/useAssistantUiStore'

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
  focusSignal?: number
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
  focusSignal = 0,
  onGenerateImage,
  statusLabel,
}: AssistantComposerProps) {
  const { t } = useTranslation()
  const composerRef = useRef<HTMLDivElement | null>(null)
  const [commandMenuOpen, setCommandMenuOpen] = useState(false)
  const commandItems = useMemo(
    () => [
      {
        command: '/plan',
        description: t('pages.assistant.composer.commandPlanDescription'),
        icon: FileText,
        label: t('pages.assistant.composer.commandPlan'),
      },
      {
        command: '/skill',
        description: t('pages.assistant.composer.commandSkillDescription'),
        icon: Wrench,
        label: t('pages.assistant.composer.commandSkill'),
      },
      {
        command: '/mcp',
        description: t('pages.assistant.composer.commandMcpDescription'),
        icon: Network,
        label: t('pages.assistant.composer.commandMcp'),
      },
      {
        command: '/web_search',
        description: t('pages.assistant.composer.commandWebSearchDescription'),
        icon: Search,
        label: t('pages.assistant.composer.commandWebSearch'),
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
  const webSearchActive = value.trimStart().startsWith('/web_search')
  const reasoningActive = reasoningSupported && reasoningEffort !== 'none'
  const resolvedToolChoice = toolChoice ?? { type: 'auto' }

  const insertCommand = (command: string) => {
    onValueChange(`${command} `)
    setCommandMenuOpen(false)
    window.setTimeout(() => {
      composerRef.current?.querySelector<HTMLTextAreaElement>('textarea')?.focus()
    })
  }

  const setToolChoiceType = (nextType: 'auto' | 'none' | 'required') => {
    setToolChoice({ type: nextType })
  }

  const handleSubmit = () => {
    const prompt = value.trim()
    if (!prompt || isRequesting || disabled) {
      return
    }

    setCommandMenuOpen(false)
    void onSubmit(prompt)
  }

  const handleValueChange = (nextValue: string) => {
    onValueChange(nextValue)
    const nextQuery = nextValue.match(/^\/([^\s/]*)$/)?.[1]?.toLowerCase()
    setCommandMenuOpen(Boolean(nextQuery !== undefined && !disabled && matchCommandItems(nextQuery).length > 0))
  }

  useEffect(() => {
    if (!focusSignal || disabled) {
      return
    }

    const timer = window.setTimeout(() => {
      composerRef.current?.querySelector<HTMLTextAreaElement>('textarea')?.focus()
    })

    return () => {
      window.clearTimeout(timer)
    }
  }, [disabled, focusSignal])

  return (
    <div ref={composerRef} className="relative flex flex-col gap-3" data-testid="assistant-composer">
      <Popover open={commandMenuOpen && matchingCommandItems.length > 0} onOpenChange={setCommandMenuOpen}>
        <PopoverAnchor asChild>
          <div
            className="rounded-2xl bg-[var(--surface-input)] p-[5px] shadow-[var(--shell-elevation)]"
            data-testid="assistant-composer-input"
          >
            <div className="flex items-end gap-2 px-4 py-2">
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon-lg"
                    disabled={disabled}
                    aria-label={t('pages.assistant.composer.advanced')}
                    className="rounded-full"
                  >
                    <Plus />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" className="rounded-2xl border-border/70">
                  <DropdownMenuGroup>
                    {commandItems.map((item) => {
                      const Icon = item.icon

                      return (
                        <DropdownMenuItem
                          key={item.command}
                          onClick={() => insertCommand(item.command)}
                        >
                          <Icon />
                          <span className="font-mono text-xs">{item.command}</span>
                          <span>{item.label}</span>
                        </DropdownMenuItem>
                      )
                    })}
                    <DropdownMenuItem onClick={onGenerateImage}>
                      <ImagePlus />
                      {t('pages.assistant.composer.generateImage')}
                    </DropdownMenuItem>
                    <DropdownMenuItem disabled>
                      <Mic />
                      {t('pages.assistant.composer.voiceCapture')}
                    </DropdownMenuItem>
                  </DropdownMenuGroup>
                </DropdownMenuContent>
              </DropdownMenu>

              <Textarea
                value={value}
                disabled={disabled}
                onChange={(event) => handleValueChange(event.currentTarget.value)}
                onKeyDown={(event) => {
                  if (event.key !== 'Enter' || event.shiftKey) {
                    return
                  }

                  event.preventDefault()
                  if (commandMenuOpen && matchingCommandItems[0]) {
                    insertCommand(matchingCommandItems[0].command)
                    return
                  }

                  handleSubmit()
                }}
                placeholder={t('pages.assistant.composer.placeholder')}
                rows={2}
                className="max-h-48 min-h-12 resize-none border-0 bg-transparent px-3 py-3 text-base shadow-none focus-visible:ring-0"
              />

              <Button
                aria-label={t('pages.assistant.composer.voiceCapture')}
                variant="ghost"
                size="icon-lg"
                className="rounded-full"
                disabled
              >
                <Mic />
              </Button>
              {isRequesting ? (
                <Button
                  aria-label={t('pages.assistant.composer.cancel')}
                  variant="secondary"
                  size="icon-lg"
                  className="rounded-full"
                  onClick={onCancel}
                >
                  <Square />
                </Button>
              ) : (
                <span data-testid="assistant-send-button">
                  <Button
                    aria-label={t('pages.assistant.composer.sendMessage')}
                    size="icon-lg"
                    className="rounded-full"
                    onClick={handleSubmit}
                    disabled={disabled || !value.trim()}
                  >
                    <Send />
                  </Button>
                </span>
              )}
            </div>
          </div>
        </PopoverAnchor>
        <PopoverContent
          align="start"
          side="top"
          className="w-[min(32rem,var(--radix-popover-trigger-width))] p-0"
          onOpenAutoFocus={(event) => event.preventDefault()}
        >
          <Command>
            <CommandList>
              <CommandEmpty>{t('pages.assistant.runtime.noData')}</CommandEmpty>
              <CommandGroup>
                {matchingCommandItems.map((item) => {
                  const Icon = item.icon

                  return (
                    <CommandItem
                      key={item.command}
                      value={item.command}
                      onSelect={() => insertCommand(item.command)}
                    >
                      <Icon />
                      <span className="min-w-0">
                        <span className="flex items-center gap-2 text-sm font-semibold">
                          <span className="font-mono text-body">{item.command}</span>
                          <span>{item.label}</span>
                        </span>
                        <span className="block truncate text-caption opacity-70">{item.description}</span>
                      </span>
                    </CommandItem>
                  )
                })}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>

      <Collapsible open={advancedPanelOpen} onOpenChange={setAdvancedPanelOpen}>
        <div className="flex flex-wrap items-center justify-between gap-3 px-2">
          <div className="flex flex-wrap items-center gap-4">
            <Button
              type="button"
              variant={webSearchActive ? 'chip' : 'quiet'}
              size="xs"
              disabled={disabled}
              aria-pressed={webSearchActive}
              data-testid="assistant-web-search-toggle"
              onClick={() => insertCommand('/web_search')}
              className={cn(
                'rounded-full',
                webSearchActive && 'text-foreground'
              )}
            >
              <Search />
              {t('pages.assistant.composer.webSearch')}
            </Button>

            <Button
              type="button"
              variant={reasoningActive ? 'chip' : 'quiet'}
              size="xs"
              disabled={disabled || !reasoningSupported}
              aria-pressed={reasoningActive}
              data-testid="assistant-reasoning-toggle"
              onClick={() => setReasoningEffort(reasoningActive ? 'none' : 'medium')}
              className={cn(
                'rounded-full',
                reasoningActive && 'text-foreground'
              )}
            >
              <WandSparkles />
              {!reasoningSupported
                ? t('pages.assistant.composer.deepThinkUnavailable')
                : reasoningActive
                  ? t('pages.assistant.composer.reasoningActive', {
                    effort: t(`pages.assistant.composer.reasoning.${reasoningEffort}`),
                  })
                  : t('pages.assistant.composer.reasoningOff')}
            </Button>

            <Button
              type="button"
              variant="quiet"
              size="xs"
              disabled={disabled}
              data-testid="assistant-generate-image-button"
              onClick={onGenerateImage}
              className="rounded-full"
            >
              <ImagePlus />
              {t('pages.assistant.composer.generateImage')}
            </Button>

            <CollapsibleTrigger asChild>
              <Button
                type="button"
                variant={advancedPanelOpen ? 'chip' : 'quiet'}
                size="xs"
                disabled={disabled}
                data-testid="assistant-advanced-toggle"
                className={cn(
                  'rounded-full',
                  advancedPanelOpen && 'text-foreground'
                )}
              >
                <SlidersHorizontal />
                {t('pages.assistant.composer.advanced')}
                <ChevronDown
                  className={cn(
                    'transition-transform',
                    advancedPanelOpen && 'rotate-180'
                  )}
                />
              </Button>
            </CollapsibleTrigger>
          </div>

          <p className="max-w-full text-micro font-medium text-muted-foreground/70">{statusLabel}</p>
        </div>

        <CollapsibleContent className="px-2 pt-3">
          <FieldGroup
            className="grid gap-3 rounded-[20px] border border-border/60 bg-[var(--surface-soft)] p-3 shadow-[inset_0_1px_0_color-mix(in_oklab,var(--foreground)_4%,transparent)] md:grid-cols-3"
            data-testid="assistant-advanced-panel"
          >
            <Field className="gap-1.5">
              <FieldLabel className="text-caption font-semibold text-muted-foreground">
                {t('pages.assistant.composer.reasoningEffort')}
              </FieldLabel>
              <Select
                value={reasoningEffort}
                disabled={disabled || !reasoningSupported}
                onValueChange={(nextValue) =>
                  setReasoningEffort(nextValue as AssistantReasoningEffort)
                }
              >
                <SelectTrigger size="sm" className="w-full text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {(['none', 'minimal', 'low', 'medium', 'high'] as const).map((item) => (
                      <SelectItem key={item} value={item}>
                        {t(`pages.assistant.composer.reasoning.${item}`)}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>

            <Field className="gap-1.5">
              <FieldLabel className="text-caption font-semibold text-muted-foreground">
                {t('pages.assistant.composer.toolChoice')}
              </FieldLabel>
              <Select
                value={resolvedToolChoice.type}
                disabled={disabled}
                onValueChange={(nextValue) =>
                  setToolChoiceType(nextValue as 'auto' | 'none' | 'required')
                }
              >
                <SelectTrigger size="sm" className="w-full text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {(['auto', 'none', 'required'] as const).map((item) => (
                      <SelectItem key={item} value={item}>
                        {t(`pages.assistant.composer.toolChoiceOptions.${item}`)}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>

            <Field className="gap-1.5">
              <FieldLabel className="text-caption font-semibold text-muted-foreground">
                {t('pages.assistant.composer.toolConcurrency')}
              </FieldLabel>
              <Select
                value={String(toolConcurrency)}
                disabled={disabled}
                onValueChange={(nextValue) => setToolConcurrency(Number(nextValue))}
              >
                <SelectTrigger size="sm" className="w-full text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {[1, 2, 3, 4].map((item) => (
                      <SelectItem key={item} value={String(item)}>
                        {item}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>

            <Field className="gap-1.5 md:col-span-3">
              <FieldLabel className="text-caption font-semibold text-muted-foreground">
                {t('pages.assistant.composer.systemPrompt')}
              </FieldLabel>
              <Textarea
                value={systemPrompt}
                disabled={disabled}
                onChange={(event) => setSystemPrompt(event.currentTarget.value)}
                className="min-h-20 resize-y text-xs leading-5"
                placeholder={t('pages.assistant.composer.systemPromptPlaceholder')}
              />
            </Field>
          </FieldGroup>
        </CollapsibleContent>
      </Collapsible>
    </div>
  )
}
