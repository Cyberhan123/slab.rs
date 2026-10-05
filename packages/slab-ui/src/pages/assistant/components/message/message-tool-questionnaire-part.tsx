"use client"

import { cn } from "@slab/ui/lib/utils"
import { CheckIcon } from "lucide-react"

import { useTranslation } from "@slab/i18n"
import { useMessageInteraction } from "../message-interaction-context"
import { ToolRow, ToolRowContent, ToolRowTrigger, toolRowIcon } from "./message-tool-row"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import { deriveState, type ToolPartLike } from "./message-tool-part"

/** The `questionnaire` tool's arguments after normalization (value resolved). */
type NormalizedQuestionnaireInput = {
  question: string
  choices: Array<{ label: string; value: string; description?: string }>
  allowMultiple: boolean
  allowCustomInput: boolean
  required: boolean
}

/** The tool-result answer payload the server fills in. */
type QuestionnaireAnswers = {
  status: string
  selected?: string[]
  custom?: string | null
}

function normalizeInput(value: unknown): NormalizedQuestionnaireInput {
  const raw = (typeof value === "object" && value !== null ? value : {}) as Record<
    string,
    unknown
  >
  const choices = Array.isArray(raw.choices)
    ? (raw.choices as Array<Record<string, unknown>>).map((choice) => ({
        label: typeof choice.label === "string" ? choice.label : "",
        value: typeof choice.value === "string" ? choice.value : String(choice.label ?? ""),
        description: typeof choice.description === "string" ? choice.description : undefined,
      }))
    : []
  return {
    question: typeof raw.question === "string" ? raw.question : "",
    choices,
    allowMultiple: raw.allow_multiple === true || raw.allowMultiple === true,
    allowCustomInput: raw.allow_custom_input === true || raw.allowCustomInput === true,
    required: raw.required === true,
  }
}

/** Accept the answer payload as an object or a JSON-encoded string. */
function parseAnswers(value: unknown): QuestionnaireAnswers | null {
  let parsed: unknown = value
  if (typeof parsed === "string") {
    try {
      parsed = JSON.parse(parsed)
    } catch {
      return null
    }
  }
  if (typeof parsed !== "object" || parsed === null) return null
  const raw = parsed as Record<string, unknown>
  return {
    status: typeof raw.status === "string" ? raw.status : "answered",
    selected: Array.isArray(raw.selected) ? raw.selected.map(String) : [],
    custom: typeof raw.custom === "string" ? raw.custom : null,
  }
}

/**
 * Renders a `questionnaire` tool call: while awaiting the user's answer the
 * row stays expanded with the question and a read-only choice list (the
 * answer UI lives in the composer's banner card); once the server fills the
 * result, the chosen answers (and any custom text) are marked in place.
 */
function MessageToolQuestionnairePart({
  part,
  kind,
  toolCallId,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  // Hooks must run unconditionally — the kind guard below returns early, so
  // useTranslation/useMessageInteraction are called before it (rules-of-hooks).
  const { t } = useTranslation()
  const { approvalStatusByItemId } = useMessageInteraction()
  if (kind !== "tool") return null

  const p = part as ToolPartLike
  const approval = toolCallId ? approvalStatusByItemId.get(toolCallId) : undefined
  const state = deriveState(p, approval)

  const input = normalizeInput(p.input)
  const answers =
    p.output !== undefined && p.output !== null && p.output !== "" ? parseAnswers(p.output) : null
  const selected = new Set(answers?.selected ?? [])

  const statusLine = !answers
    ? t("pages.assistant.questionnaire.waiting")
    : answers.status === "timeout"
      ? t("pages.assistant.questionnaire.timeout")
      : answers.status === "skipped"
        ? t("pages.assistant.questionnaire.skipped")
        : t("pages.assistant.questionnaire.answered")

  return (
    // Expanded while the user has not answered yet (the in-stream card is the
    // prompt; the composer banner card is the input surface).
    <ToolRow defaultOpen={answers === null}>
      <ToolRowTrigger
        icon={toolRowIcon("questionnaire")}
        label="Ask"
        detail={input.question}
        state={state}
      />
      <ToolRowContent>
        <div data-testid="assistant-tool-questionnaire" className="space-y-2">
          <div className="text-foreground text-sm">{input.question}</div>
          <ul className="space-y-1">
            {input.choices.map((choice) => {
              const checked = selected.has(choice.value)
              return (
                <li
                  key={choice.value}
                  className={cn(
                    "flex items-start gap-2 rounded-md px-2 py-1.5 text-xs",
                    checked ? "bg-accent text-accent-foreground" : "bg-muted/50",
                  )}
                >
                  <span
                    className={cn(
                      "mt-0.5 flex size-4 shrink-0 items-center justify-center border",
                      input.allowMultiple ? "rounded-sm" : "rounded-full",
                      checked ? "border-primary bg-primary text-primary-foreground" : "border-border",
                    )}
                  >
                    {checked ? <CheckIcon className="size-3" /> : null}
                  </span>
                  <span className="min-w-0 flex-1">
                    {choice.label}
                    {choice.description ? (
                      <span className="block text-muted-foreground">{choice.description}</span>
                    ) : null}
                  </span>
                </li>
              )
            })}
            {answers?.custom ? (
              <li className="rounded-md bg-muted/50 px-2 py-1.5 text-xs">
                <span className="text-muted-foreground">
                  {t("pages.assistant.questionnaire.customPlaceholder")}:
                </span>{" "}
                <span className="text-foreground">{answers.custom}</span>
              </li>
            ) : null}
          </ul>
          <p className="text-muted-foreground text-xs">{statusLine}</p>
        </div>
      </ToolRowContent>
    </ToolRow>
  )
}

export default MessageToolQuestionnairePart
