"use client"

import { useRef, useState, type SubmitEvent } from "react"
import type { FileUIPart } from "ai"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupTextarea,
} from "@slab/components/input-group"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  DropdownMenuGroup,
  DropdownMenuLabel,
} from "@slab/components/dropdown-menu"
import { Button } from "@slab/components/button"
import { Spinner } from "@slab/components/spinner"
import {
  ToggleGroup,
  ToggleGroupItem,
} from "@slab/components/toggle-group"
import { Switch } from "@slab/components/switch"
import { useTranslation } from "@slab/i18n"
import {
  ArrowUpIcon,
  Brain,
  Check,
  Dot,
  File,
  ListChecksIcon,
  PaperclipIcon,
  PlusIcon,
  ShieldCheck,
  Slash,
  Sparkle,
  SquareIcon,
  XIcon,
} from "lucide-react"

import type {
  ApprovalScope,
  CommandInfo,
  InteractionMode,
  PermissionMode,
  ReasoningEffort,
} from "../lib/harness"
import type { ApprovalRequest } from "../hooks/use-harness-conversation"
import { resolveCommandDispatch } from "../lib/assistant-commands"
import { ApprovalCard } from "./approval-banner"

/** Per-session permission modes offered in the composer. */
const PERMISSION_MODES: ReadonlyArray<{ value: PermissionMode; label: string }> = [
  { value: "request_approval", label: "pages.assistant.composer.permission.requestApproval" },
  { value: "approve_for_me", label: "pages.assistant.composer.permission.approveForMe" },
  { value: "full_control", label: "pages.assistant.composer.permission.fullControl" },
  { value: "custom", label: "pages.assistant.composer.permission.custom" },
]

/** Per-session interaction modes offered in the composer (orthogonal to permission). */
const INTERACTION_MODES: ReadonlyArray<{ value: InteractionMode; label: string }> = [
  { value: "default", label: "pages.assistant.composer.interaction.default" },
  { value: "plan", label: "pages.assistant.composer.interaction.plan" },
]

type EffortLevel = "low" | "medium" | "high"

interface Attachment {
  id: string
  file: File
  previewUrl: string
}

export type SenderSubmitOptions = {
  files: FileUIPart[]
  effort: ReasoningEffort
  permissionMode: PermissionMode
  interactionMode: InteractionMode
}

type SenderProps = {
  onSubmit: (
    message: string,
    options: SenderSubmitOptions,
    event?: SubmitEvent<HTMLFormElement>,
  ) => Promise<void> | void
  /** When provided while `loading`, the submit button becomes a Stop control. */
  onStop?: () => void
  loading?: boolean
  /** Pending human-approval requests rendered in a slot above the textarea. */
  approvals?: ApprovalRequest[]
  onResolveApproval?: (itemId: string, approved: boolean, scope: ApprovalScope) => Promise<void> | void
  /** Command registry snapshot driving the `/`-menu (`command/list`). */
  commands: CommandInfo[]
  /** Current interaction mode (lifted from the conversation hook). */
  interactionMode: InteractionMode
  /** Set the interaction mode (absolute); `/plan` toggles via this. */
  onInteractionModeChange: (mode: InteractionMode) => void
}

function fileToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.addEventListener("load", () => resolve(reader.result as string))
    reader.addEventListener("error", () => reject(reader.error))
    reader.readAsDataURL(file)
  })
}

function Sender({
  onSubmit,
  onStop,
  loading = false,
  approvals,
  onResolveApproval,
  commands,
  interactionMode,
  onInteractionModeChange,
}: SenderProps) {
  const { t } = useTranslation()
  const [value, setValue] = useState("")
  const [attachments, setAttachments] = useState<Attachment[]>([])
  const [thinkingEnabled, setThinkingEnabled] = useState(false)
  const [effortLevel, setEffortLevel] = useState<EffortLevel>("high")
  const [permissionMode, setPermissionMode] = useState<PermissionMode>("request_approval")
  const [commandMenuOpen, setCommandMenuOpen] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  // The "/" command menu opens both from the toolbar button and from typing a
  // leading "/", as one unified popover above the input.
  const isSlashCommand = value.trimStart().startsWith("/")

  const effort: ReasoningEffort = thinkingEnabled ? effortLevel : "off"
  const isGenerating = loading
  const showStop = isGenerating && onStop
  const canSend = !isGenerating && (value.trim().length > 0 || attachments.length > 0)

  const addFiles = (fileList: FileList | File[] | null) => {
    if (!fileList) return
    const incoming = Array.from(fileList)
    if (incoming.length === 0) return
    const next = incoming.map((file) => ({
      id:
        typeof crypto !== "undefined" && "randomUUID" in crypto
          ? crypto.randomUUID()
          : `${file.name}-${file.size}-${Math.random().toString(36).slice(2)}`,
      file,
      previewUrl: URL.createObjectURL(file),
    }))
    setAttachments((prev) => [...prev, ...next])
  }

  const removeAttachment = (id: string) => {
    setAttachments((prev) => {
      const target = prev.find((item) => item.id === id)
      if (target) URL.revokeObjectURL(target.previewUrl)
      return prev.filter((item) => item.id !== id)
    })
  }

  const handleSubmit = async (event?: SubmitEvent<HTMLFormElement>) => {
    const message = value.trim()
    if (!message && attachments.length === 0) return
    if (isGenerating) return

    // `/plan` toggles the client-side Plan interaction mode (no message sent).
    // The server is the source of truth via the `turn/start` interactionMode.
    const dispatch = resolveCommandDispatch(value, commands)
    if (dispatch.action === "togglePlan") {
      onInteractionModeChange(interactionMode === "plan" ? "default" : "plan")
      setValue("")
      setCommandMenuOpen(false)
      return
    }

    const files: FileUIPart[] = await Promise.all(
      attachments.map(async (item) => ({
        type: "file" as const,
        mediaType: item.file.type || "application/octet-stream",
        filename: item.file.name,
        url: await fileToDataUrl(item.file),
      })),
    )

    await onSubmit(message, { files, effort, permissionMode, interactionMode }, event)

    setValue("")
    setCommandMenuOpen(false)
    for (const item of attachments) URL.revokeObjectURL(item.previewUrl)
    setAttachments([])
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault()
        void handleSubmit(e)
      }}
      onDragOver={(e) => {
        e.preventDefault()
      }}
      onDrop={(e) => {
        e.preventDefault()
        addFiles(e.dataTransfer.files)
      }}
      className="w-full"
    >
      <input
        ref={fileInputRef}
        type="file"
        multiple
        hidden
        aria-label="Attach files"
        onChange={(e) => {
          addFiles(e.target.files)
          e.target.value = ""
        }}
      />

      {approvals && approvals.length > 0 && onResolveApproval ? (
        <div className="mb-2 space-y-2">
          {approvals.map((approval) => (
            <ApprovalCard
              key={approval.itemId}
              approval={approval}
              onResolve={onResolveApproval}
            />
          ))}
        </div>
      ) : null}

      {attachments.length > 0 ? (
        <div className="mb-2 flex flex-wrap gap-2">
          {attachments.map((item) => (
            <div
              key={item.id}
              className="flex items-center gap-1 rounded-md border bg-muted/40 px-2 py-1 text-xs"
            >
              <PaperclipIcon className="size-3 text-muted-foreground" />
              <span className="max-w-40 truncate">{item.file.name}</span>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="size-4"
                aria-label={t("pages.assistant.message.cancelEdit")}
                onClick={() => removeAttachment(item.id)}
              >
                <XIcon className="size-3" />
              </Button>
            </div>
          ))}
        </div>
      ) : null}

      <InputGroup>
        <InputGroupTextarea
          aria-label="Message"
          data-testid="assistant-composer-input"
          disabled={loading}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault()
              event.currentTarget.form?.requestSubmit()
            }
          }}
          onPaste={(event) => {
            const files = event.clipboardData?.files
            if (files && files.length > 0) addFiles(files)
          }}
          placeholder={t("pages.assistant.composer.placeholder")}
          rows={3}
          value={value}
        />
        <InputGroupAddon align="block-end" className="pt-1">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <InputGroupButton
                aria-label="Add files"
                type="button"
                size="icon-sm"
                variant="outline"
              >
                <PlusIcon />
              </InputGroupButton>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" side="top" className="w-44">
              <DropdownMenuItem
                onSelect={() => {
                  fileInputRef.current?.click()
                }}
              >
                <PaperclipIcon />
                {t("pages.assistant.runtime.workspace")}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem>
                <File />
                {t("pages.assistant.composer.commandMcp")}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <DropdownMenu
            open={commandMenuOpen || isSlashCommand}
            onOpenChange={setCommandMenuOpen}
            modal={false}
          >
            <DropdownMenuTrigger asChild>
              <InputGroupButton
                aria-label="Commands"
                type="button"
                size="icon-sm"
                variant="outline"
              >
                <Slash />
              </InputGroupButton>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" side="top" className="w-44">
              <DropdownMenuGroup>
                <DropdownMenuLabel>Model</DropdownMenuLabel>
                <DropdownMenuItem
                  onSelect={(event) => {
                    // Keep the menu open so the embedded ToggleGroup stays interactive.
                    event.preventDefault()
                  }}
                >
                  <Brain />
                  {t("pages.assistant.composer.reasoningEffort")}
                  <ToggleGroup
                    variant="outline"
                    type="single"
                    value={effortLevel}
                    onValueChange={(level) => {
                      if (level) {
                        setEffortLevel(level as EffortLevel)
                        setThinkingEnabled(true)
                      }
                    }}
                  >
                    <ToggleGroupItem value="low" aria-label="Toggle low">
                      <Dot /> {t("pages.assistant.composer.reasoning.low")}
                    </ToggleGroupItem>
                    <ToggleGroupItem value="medium" aria-label="Toggle medium">
                      <Dot /> {t("pages.assistant.composer.reasoning.medium")}
                    </ToggleGroupItem>
                    <ToggleGroupItem value="high" aria-label="Toggle high">
                      <Dot /> {t("pages.assistant.composer.reasoning.high")}
                    </ToggleGroupItem>
                  </ToggleGroup>
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  onSelect={(event) => {
                    event.preventDefault()
                    setThinkingEnabled((prev) => !prev)
                  }}
                >
                  <Sparkle />
                  {t("pages.assistant.composer.deepThink")}
                  <Switch
                    checked={thinkingEnabled}
                    onCheckedChange={setThinkingEnabled}
                  />
                </DropdownMenuItem>
              </DropdownMenuGroup>
              <DropdownMenuGroup>
                <DropdownMenuLabel>
                  {t("pages.assistant.composer.permission.title")}
                </DropdownMenuLabel>
                {PERMISSION_MODES.map((mode) => (
                  <DropdownMenuItem
                    key={mode.value}
                    data-testid={`assistant-permission-mode-${mode.value}`}
                    onSelect={(event) => {
                      event.preventDefault()
                      setPermissionMode(mode.value)
                    }}
                  >
                    <ShieldCheck />
                    {t(mode.label)}
                    {permissionMode === mode.value ? <Check className="ml-auto size-3.5" /> : null}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuGroup>
              <DropdownMenuGroup>
                <DropdownMenuLabel>
                  {t("pages.assistant.composer.interaction.title")}
                </DropdownMenuLabel>
                {INTERACTION_MODES.map((mode) => (
                  <DropdownMenuItem
                    key={mode.value}
                    data-testid={`assistant-interaction-mode-${mode.value}`}
                    onSelect={(event) => {
                      event.preventDefault()
                      onInteractionModeChange(mode.value)
                    }}
                  >
                    <ListChecksIcon />
                    {t(mode.label)}
                    {interactionMode === mode.value ? <Check className="ml-auto size-3.5" /> : null}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuGroup>
              <DropdownMenuGroup>
                <DropdownMenuLabel>
                  {t("pages.assistant.composer.commandSkill")}
                </DropdownMenuLabel>
                {commands.map((cmd) => {
                  const seed = `/${cmd.name}`
                  return (
                    <DropdownMenuItem
                      key={cmd.name}
                      title={cmd.description}
                      onSelect={(event) => {
                        event.preventDefault()
                        // `/plan` toggles Plan interaction mode directly (no
                        // seeding, no message). Control commands seed the exact
                        // trigger (ready to submit); Prompt/Render commands
                        // prefix the input for further typing.
                        if (cmd.name === "plan") {
                          onInteractionModeChange(interactionMode === "plan" ? "default" : "plan")
                          setCommandMenuOpen(false)
                          return
                        }
                        if (cmd.kind === "control") {
                          setValue(seed)
                        } else {
                          setValue((prev) => (prev.startsWith(seed) ? prev : `${seed} ${prev}`))
                        }
                      }}
                    >
                      {seed}
                    </DropdownMenuItem>
                  )
                })}
              </DropdownMenuGroup>
            </DropdownMenuContent>
          </DropdownMenu>
          {showStop ? (
            <InputGroupButton
              aria-label={t("pages.assistant.composer.stopGeneratingResponse")}
              type="button"
              variant="outline"
              size="icon-sm"
              className="ml-auto"
              onClick={() => onStop?.()}
            >
              <SquareIcon className="size-4" />
              <span className="sr-only">
                {t("pages.assistant.composer.stopGeneratingResponse")}
              </span>
            </InputGroupButton>
          ) : (
            <InputGroupButton
              aria-label="Send"
              data-testid="assistant-send-button"
              type="submit"
              variant="default"
              size="icon-sm"
              disabled={!canSend}
              className="ml-auto"
            >
              {isGenerating ? <Spinner /> : <ArrowUpIcon />}
              <span className="sr-only">
                {t("pages.assistant.composer.sendMessage")}
              </span>
            </InputGroupButton>
          )}
        </InputGroupAddon>
      </InputGroup>
    </form>
  )
}

export default Sender
