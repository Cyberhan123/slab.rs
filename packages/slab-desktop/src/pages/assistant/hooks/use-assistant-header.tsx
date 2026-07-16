import { useMemo, useRef } from "react"
import { MessageCirclePlus } from "lucide-react"

import { useTranslation } from "@slab/i18n"
import { Button } from "@slab/components/button"
import { useHeader } from "@/hooks/use-header"

import type { ModelOption } from "../lib/assistant-page-state"

type UseAssistantHeaderOptions = {
  modelOptions: ModelOption[]
  selectedModelId: string
  modelLoading: boolean
  isSessionBusy: boolean
  isSessionBootstrapping: boolean
  pendingModelSwitchId: string | null
  onModelPickerChange: (nextModelId: string) => void
  onOpenSessionSheet: () => void
  /** Create a fresh assistant session (the header "new session" button). */
  onNewSession: () => void
}

export function useAssistantHeader({
  modelOptions,
  selectedModelId,
  modelLoading,
  isSessionBusy,
  isSessionBootstrapping,
  pendingModelSwitchId,
  onModelPickerChange,
  onOpenSessionSheet,
  onNewSession,
}: UseAssistantHeaderOptions) {
  const { t } = useTranslation()

  const headerModelPicker = useMemo(
    () => ({
      disabled:
        modelLoading ||
        isSessionBusy ||
        isSessionBootstrapping ||
        Boolean(pendingModelSwitchId) ||
        modelOptions.length === 0,
      emptyLabel: t("pages.assistant.modelPicker.emptyLabel"),
      groupLabel: t("pages.assistant.modelPicker.groupLabel"),
      loading: modelLoading,
      onChange: onModelPickerChange,
      options: modelOptions.map((model) => ({
        id: model.id,
        label: model.label,
      })),
      placeholder: t("pages.assistant.modelPicker.placeholder"),
      value: selectedModelId,
    }),
    [
      isSessionBootstrapping,
      isSessionBusy,
      modelLoading,
      modelOptions,
      onModelPickerChange,
      pendingModelSwitchId,
      selectedModelId,
      t,
    ]
  )

  const headerHistoryButton = useMemo(
    () => ({
      ariaLabel: t("pages.assistant.sessionSheet.title"),
      disabled: isSessionBootstrapping,
      onClick: onOpenSessionSheet,
      title: t("pages.assistant.sessionSheet.title"),
    }),
    [isSessionBootstrapping, onOpenSessionSheet, t]
  )

  const newSessionLabel = t("pages.assistant.header.newSession")
  const newSessionDisabled = isSessionBusy || isSessionBootstrapping
  // The header slot stores a ReactNode and compares by reference, so the element
  // MUST stay referentially stable across renders — otherwise it re-registers
  // every render, which loops (setState in the layout effect → re-render → …).
  // `onNewSession` can change identity when its upstream callback does, so route
  // it through a ref: the element's onClick closure is created once and is
  // stable; only primitive deps (`disabled`, label) can recreate it (legitimately).
  const onNewSessionRef = useRef(onNewSession)
  onNewSessionRef.current = onNewSession
  const headerNewSessionButton = useMemo(
    () => (
      <Button
        aria-label={newSessionLabel}
        data-testid="header-new-session-control"
        disabled={newSessionDisabled}
        onClick={() => onNewSessionRef.current()}
        size="icon-sm"
        title={newSessionLabel}
        type="button"
        variant="quiet"
        className="size-8 shrink-0 rounded-full text-[color:var(--shell-rail-label)] hover:bg-glass-bg-strong hover:text-[color:var(--shell-title)]"
      >
        <MessageCirclePlus data-icon="inline-start" />
      </Button>
    ),
    [newSessionDisabled, newSessionLabel],
  )

  useHeader({
    history: headerHistoryButton,
    select: headerModelPicker,
    right: headerNewSessionButton,
  })
}
