"use client"

import { useRef, useState, type ReactNode, type SubmitEvent } from "react"
import type { FileUIPart } from "ai"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupTextarea,
} from "@slab/components/input-group"
import { useSlab } from "@slab/ui/provider/slab-provider"

import { useVoiceInput } from "../lib/use-voice-input"
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
  Loader2,
  Mic,
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
  PermissionMode,
  ReasoningEffort,
} from "@slab/api/harness"
import type { ApprovalRequest } from "@slab/core/harness"
import { resolveCommandDispatch } from "../lib/assistant-commands"
import { ApprovalCard } from "./approval-banner"

/** Per-session permission modes offered in the composer. */
const PERMISSION_MODES: ReadonlyArray<{ value: PermissionMode; label: string }> = [
  { value: "request_approval", label: "pages.assistant.composer.permission.requestApproval" },
  { value: "approve_for_me", label: "pages.assistant.composer.permission.approveForMe" },
  { value: "full_control", label: "pages.assistant.composer.permission.fullControl" },
  { value: "custom", label: "pages.assistant.composer.permission.custom" },
]

type EffortLevel = "low" | "medium" | "high"

interface Attachment {
  id: string
  /** Browser File (web / paste / drop) OR a native filesystem path (Tauri picker). */
  file: File | string
  name: string
  mediaType: string
  previewUrl: string
}

/** Infer an image media type from a filesystem path's extension. */
function imageMediaTypeFromPath(path: string): string {
  const ext = path.split(/[/\\]/).pop()?.split(".").pop()?.toLowerCase() ?? ""
  switch (ext) {
    case "png":
      return "image/png"
    case "jpg":
    case "jpeg":
      return "image/jpeg"
    case "gif":
      return "image/gif"
    case "webp":
      return "image/webp"
    case "bmp":
      return "image/bmp"
    default:
      return "application/octet-stream"
  }
}

function basename(path: string): string {
  return path.split(/[/\\]/).pop() ?? path
}

function attachmentId(seed: string): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${seed}-${Math.random().toString(36).slice(2)}`
}

export type SenderSubmitOptions = {
  files: FileUIPart[]
  effort: ReasoningEffort
  permissionMode: PermissionMode
  /** Built-in agent type for this turn (`"plan"` when plan mode is active). */
  agentType?: "plan"
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
  /**
   * Allow submitting WHILE a turn runs (steering): the submit routes to the
   * queued steering path instead of a second AI-SDK stream. The single submit
   * button becomes Stop while generating with nothing to steer (empty
   * composer, or a non-steerable turn) and stays Send otherwise.
   */
  steerable?: boolean
  /** Pending human-approval requests rendered in a slot above the textarea. */
  approvals?: ApprovalRequest[]
  onResolveApproval?: (itemId: string, approved: boolean, scope: ApprovalScope) => Promise<void> | void
  /** Command registry snapshot driving the `/`-menu (`command/list`). */
  commands: CommandInfo[]
  /** Whether plan mode is active (turn runs as the read-only plan agent). */
  planMode: boolean
  /** Toggle plan mode on/off; `/plan` and the plan chip's X use this. */
  onPlanModeChange: (enabled: boolean) => void
  /**
   * Extra control rendered in the bottom toolbar next to the permission-mode
   * toggle — the assistant page passes the live workspace selector here.
   */
  workspaceSlot?: ReactNode
  /**
   * Seed text for the composer (claimed once on mount, never re-synced). The
   * new-chat landing uses it to prefill from a staged workspace handoff draft.
   */
  initialValue?: string
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
  steerable = false,
  approvals,
  onResolveApproval,
  commands,
  planMode,
  onPlanModeChange,
  workspaceSlot,
  initialValue,
}: SenderProps) {
  const { t } = useTranslation()
  const { ports } = useSlab()
  const isTauri = ports.platformInfo.desktop
  const [value, setValue] = useState(initialValue ?? "")
  const [attachments, setAttachments] = useState<Attachment[]>([])
  const [thinkingEnabled, setThinkingEnabled] = useState(false)
  const [effortLevel, setEffortLevel] = useState<EffortLevel>("high")
  const [permissionMode, setPermissionMode] = useState<PermissionMode>("request_approval")
  const [commandMenuOpen, setCommandMenuOpen] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  // Voice input transcribes speech into the composer via the shared
  // whisper/parakeet backends. The hook is always called (Rules of Hooks); the
  // button is rendered only on Tauri — the path-based transcribe endpoint and
  // the temp-file staging are host-only, so voice input is desktop-only.
  const voiceInput = useVoiceInput({
    onTranscript: (text) =>
      setValue((prev) => (prev.trim() ? `${prev.replace(/\s+$/, "")} ${text}` : text)),
  })

  // The "/" command menu opens both from the toolbar button and from typing a
  // leading "/", as one unified popover above the input.
  const isSlashCommand = value.trimStart().startsWith("/")

  const effort: ReasoningEffort = thinkingEnabled ? effortLevel : "off"
  const isGenerating = loading
  const showStop = Boolean(isGenerating && onStop)
  // Steerable turns keep submit enabled while generating; plain turns lock it.
  const canSend =
    (!isGenerating || steerable) && (value.trim().length > 0 || attachments.length > 0)
  // Single-button state machine: while generating, the button is Stop when
  // there is nothing to steer (empty composer or non-steerable turn) and Send
  // otherwise; idle turns always show Send.
  const stopMode = showStop && !canSend

  const addFiles = (fileList: FileList | File[] | null) => {
    if (!fileList) return
    const incoming = Array.from(fileList)
    if (incoming.length === 0) return
    const next = incoming.map((file) => ({
      id: attachmentId(file.name),
      file,
      name: file.name,
      mediaType: file.type || "application/octet-stream",
      previewUrl: URL.createObjectURL(file),
    }))
    setAttachments((prev) => [...prev, ...next])
  }

  /** Tauri-only: attach native file paths from the OS dialog (no base64 round-trip). */
  const addPaths = (paths: string[]) => {
    if (paths.length === 0) return
    const next = paths.map((path) => ({
      id: attachmentId(path),
      file: path,
      name: basename(path),
      mediaType: imageMediaTypeFromPath(path),
      previewUrl: ports.imageSrc.resolve(path),
    }))
    setAttachments((prev) => [...prev, ...next])
  }

  const removeAttachment = (id: string) => {
    setAttachments((prev) => {
      const target = prev.find((item) => item.id === id)
      // Only blob: URLs (web File previews) need revocation; Tauri asset URLs
      // (convertFileSrc) are not object URLs.
      if (target && target.previewUrl.startsWith("blob:")) URL.revokeObjectURL(target.previewUrl)
      return prev.filter((item) => item.id !== id)
    })
  }

  const handleSubmit = async (event?: SubmitEvent<HTMLFormElement>) => {
    const message = value.trim()
    if (!message && attachments.length === 0) return
    if (isGenerating && !steerable) return

    // `/plan` toggles client-side plan mode (no message sent). The server runs
    // the next turn as the read-only plan agent via `turn/start` agentType.
    const dispatch = resolveCommandDispatch(value, commands)
    if (dispatch.action === "togglePlan") {
      onPlanModeChange(!planMode)
      setValue("")
      setCommandMenuOpen(false)
      return
    }

    const files: FileUIPart[] = await Promise.all(
      attachments.map(async (item) => {
        // Native path (Tauri picker): send the path verbatim — buildTurnInput
        // maps it to `localImage` and the server reads the file directly.
        if (typeof item.file === "string") {
          return {
            type: "file" as const,
            mediaType: item.mediaType,
            filename: item.name,
            url: item.file,
          }
        }
        return {
          type: "file" as const,
          mediaType: item.file.type || "application/octet-stream",
          filename: item.file.name,
          url: await fileToDataUrl(item.file),
        }
      }),
    )

    await onSubmit(
      message,
      { files, effort, permissionMode, agentType: planMode ? "plan" : undefined },
      event,
    )

    setValue("")
    setCommandMenuOpen(false)
    for (const item of attachments) {
      if (item.previewUrl.startsWith("blob:")) URL.revokeObjectURL(item.previewUrl)
    }
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
              <span className="max-w-40 truncate">{item.name}</span>
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
          disabled={loading && !steerable}
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
                  // On Tauri the OS dialog yields native paths (no base64
                  // round-trip); on web we fall back to the hidden file input.
                  if (isTauri) {
                    void (async () => {
                      const picked = await ports.fileDialog.pickFiles({
                        multiple: true,
                        filters: [
                          {
                            name: "Images",
                            extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp"],
                          },
                        ],
                      })
                      addPaths(
                        picked
                          .map((entry) => entry.path)
                          .filter((path): path is string => typeof path === "string"),
                      )
                    })()
                  } else {
                    fileInputRef.current?.click()
                  }
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
          {isTauri ? (
            <InputGroupButton
              aria-label={
                voiceInput.state === "recording"
                  ? "Stop recording"
                  : voiceInput.state === "transcribing"
                    ? "Transcribing"
                    : "Voice input"
              }
              type="button"
              size="icon-sm"
              variant={
                voiceInput.state === "recording" ? "default" : "outline"
              }
              disabled={voiceInput.busy}
              onClick={() => {
                void voiceInput.toggle()
              }}
            >
              {voiceInput.state === "transcribing" ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Mic className="size-4" />
              )}
            </InputGroupButton>
          ) : null}
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
            <DropdownMenuContent align="start" side="top">
              <DropdownMenuGroup>
                <DropdownMenuLabel>
                  {t("common.fields.model")}
                </DropdownMenuLabel>
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
                        // `/plan` toggles plan mode directly (no seeding, no
                        // message). Control commands seed the exact trigger
                        // (ready to submit); Prompt/Render commands prefix the
                        // input for further typing.
                        if (cmd.name === "plan") {
                          onPlanModeChange(!planMode)
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
          <DropdownMenu modal={false}>
            <DropdownMenuTrigger asChild>
              <InputGroupButton
                aria-label={t("pages.assistant.composer.permission.title")}
                data-testid="assistant-permission-mode-trigger"
                type="button"
                variant="outline"
                size="sm"
              >
                <ShieldCheck className="size-4" />
                {t(
                  PERMISSION_MODES.find((m) => m.value === permissionMode)?.label ??
                    PERMISSION_MODES[0].label,
                )}
              </InputGroupButton>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" side="top">
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
            </DropdownMenuContent>
          </DropdownMenu>
          {workspaceSlot}
          <div className="ml-auto flex items-center gap-1">
            <InputGroupButton
              aria-label={stopMode ? t("pages.assistant.composer.stopGeneratingResponse") : "Send"}
              data-testid="assistant-send-button"
              data-mode={stopMode ? "stop" : "send"}
              type={stopMode ? "button" : "submit"}
              variant={stopMode ? "outline" : "default"}
              size="icon-sm"
              disabled={!stopMode && !canSend}
              onClick={stopMode ? () => onStop?.() : undefined}
            >
              {stopMode ? (
                <SquareIcon className="size-4" />
              ) : isGenerating && !canSend ? (
                <Spinner />
              ) : (
                <ArrowUpIcon />
              )}
              <span className="sr-only">
                {stopMode
                  ? t("pages.assistant.composer.stopGeneratingResponse")
                  : t("pages.assistant.composer.sendMessage")}
              </span>
            </InputGroupButton>
            {planMode ? (
              <InputGroupButton
                aria-label={t("pages.assistant.planMode.exit")}
                data-testid="assistant-plan-mode-chip"
                type="button"
                variant="outline"
                size="sm"
                onClick={() => onPlanModeChange(false)}
              >
                <ListChecksIcon className="size-3.5" />
                {t("pages.assistant.composer.interaction.plan")}
                <XIcon className="size-3" />
              </InputGroupButton>
            ) : null}
          </div>
        </InputGroupAddon>
      </InputGroup>
    </form>
  )
}

export default Sender
