"use client"

import { Button } from "@slab/components/button"
import { Spinner } from "@slab/components/spinner"
import {
  Questionnaire,
  QuestionnaireActions,
  QuestionnaireChoice,
  QuestionnaireChoiceDescription,
  QuestionnaireChoices,
  QuestionnaireError,
  QuestionnaireInput,
  QuestionnaireItem,
  QuestionnaireSubmit,
  QuestionnaireTitle,
} from "@slab/components/questionnaire"
import { useTranslation } from "@slab/i18n"
import { MessageCircleQuestionIcon } from "lucide-react"
import { useState } from "react"

import type { QuestionnaireRequest } from "@slab/core/harness"

/** Answer payload sent back through `questionnaire/resolve`. */
export type QuestionnaireAnswerPayload = {
  selected: string[]
  custom: string | null
  skipped?: boolean
}

/**
 * The answer card for a pending `questionnaire` tool call, rendered in the
 * composer's banner slot (above the input, alongside approval cards).
 *
 * Unlike the approval card (which resolves on the first click), a
 * questionnaire supports multiple selection and a free-text answer, so it
 * submits explicitly: choices toggle, the custom input (when offered) fills
 * freely, and Submit (or Skip, when the question is optional) resolves the
 * call. Selection lives in React state rather than FormData — the primitive
 * names the custom input after the same item, which would pollute
 * `getAll("answers")` once filled.
 */
export function QuestionnaireCard({
  request,
  onResolve,
}: {
  request: QuestionnaireRequest
  onResolve: (itemId: string, answers: QuestionnaireAnswerPayload) => Promise<void> | void
}) {
  const { t } = useTranslation()
  const [pending, setPending] = useState(false)
  const [invalid, setInvalid] = useState(false)
  const [selected, setSelected] = useState<string[]>([])
  const [customText, setCustomText] = useState("")

  const resolve = async (answers: QuestionnaireAnswerPayload) => {
    setPending(true)
    try {
      await onResolve(request.itemId, answers)
    } finally {
      setPending(false)
    }
  }

  const toggleChoice = (value: string, checked: boolean) => {
    setInvalid(false)
    setSelected((prev) => {
      if (request.allowMultiple) {
        return checked ? [...prev, value] : prev.filter((v) => v !== value)
      }
      // Single-select radios: the newly checked choice replaces the previous.
      return checked ? [value] : prev.filter((v) => v !== value)
    })
  }

  const custom = customText.trim() ? customText.trim() : null

  return (
    <div
      className="rounded-md border bg-card p-3"
      data-testid="assistant-questionnaire-card"
    >
      <div className="flex items-center gap-2 text-sm font-medium">
        <MessageCircleQuestionIcon className="size-4 text-primary" />
        <span>{t("pages.assistant.questionnaire.title")}</span>
      </div>

      <Questionnaire
        noValidate
        onSubmit={(event) => {
          // The questionnaire Root is a form; Submit triggers it. Validation
          // is ours (noValidate skips the primitive's native checks).
          event.preventDefault()
          if (selected.length === 0 && custom === null) {
            if (request.required) {
              setInvalid(true)
              return
            }
            // Optional with nothing picked resolves as skipped.
            void resolve({ selected: [], custom: null, skipped: true })
            return
          }
          setInvalid(false)
          void resolve({ selected, custom })
        }}
      >
        <QuestionnaireItem name="answers" multiple={request.allowMultiple} invalid={invalid}>
          <QuestionnaireTitle className="mt-2 text-pretty text-sm font-medium text-foreground">
            {request.question}
          </QuestionnaireTitle>
          <QuestionnaireChoices className="mt-1.5 grid-cols-1 gap-1.5">
            {request.choices.map((choice) => (
              <QuestionnaireChoice
                key={choice.value}
                value={choice.value}
                disabled={pending}
                data-testid={`assistant-questionnaire-choice-${choice.value}`}
                className="gap-3 rounded-md border border-border/60 px-3 py-2 text-sm transition-colors hover:bg-accent/50 data-checked:border-primary/60 data-checked:bg-accent"
                onChange={(event) => toggleChoice(choice.value, event.target.checked)}
              >
                {choice.label}
                {choice.description ? (
                  <QuestionnaireChoiceDescription className="text-xs text-muted-foreground">
                    {choice.description}
                  </QuestionnaireChoiceDescription>
                ) : null}
              </QuestionnaireChoice>
            ))}
          </QuestionnaireChoices>
          {request.allowCustomInput ? (
            <QuestionnaireInput
              type="text"
              disabled={pending}
              value={customText}
              placeholder={t("pages.assistant.questionnaire.customPlaceholder")}
              className="mt-1.5 min-h-0 rounded-md border border-border/60 px-3 py-2 text-sm"
              onChange={(event) => {
                setInvalid(false)
                setCustomText(event.target.value)
              }}
            />
          ) : null}
          <QuestionnaireError className="mt-1.5 text-xs">
            {t("pages.assistant.questionnaire.required")}
          </QuestionnaireError>
        </QuestionnaireItem>
        {/* Keep the base 3-column grid: the Submit wrapper pins itself to
            col-start-3; Skip (or its placeholder span) occupies col-start-1. */}
        <QuestionnaireActions className="mt-3 min-h-0">
          {request.required ? (
            <span />
          ) : (
            <Button
              type="button"
              variant="outline"
              size="sm"
              data-testid="assistant-questionnaire-skip"
              disabled={pending}
              className="col-start-1 row-start-1 justify-self-start"
              onClick={() => void resolve({ selected: [], custom: null, skipped: true })}
            >
              {t("pages.assistant.questionnaire.skip")}
            </Button>
          )}
          <QuestionnaireSubmit
            size="sm"
            data-testid="assistant-questionnaire-submit"
            disabled={pending}
          >
            {pending ? <Spinner className="size-3.5" /> : null}
            {t("pages.assistant.questionnaire.submit")}
          </QuestionnaireSubmit>
        </QuestionnaireActions>
      </Questionnaire>
    </div>
  )
}
