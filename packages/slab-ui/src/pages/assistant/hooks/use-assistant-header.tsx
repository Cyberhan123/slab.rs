import { useEffect, useMemo, useRef } from "react"
import { MessageCirclePlus } from "lucide-react"

import { useTranslation } from "@slab/i18n"
import { Button } from "@slab/components/button"
import { useHeader } from "@slab/ui/hooks/use-header"
import type { HeaderSelectConfig } from "@slab/ui/layouts/header"

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
    () => {
      const toOption = (model: ModelOption) => ({ id: model.id, label: model.label })
      const groups: HeaderSelectConfig["options"] = []
      const localModels = modelOptions.filter((model) => model.source === "local")
      const cloudModels = modelOptions.filter((model) => model.source === "cloud")
      if (localModels.length > 0) {
        groups.push({
          id: "local",
          label: t("pages.assistant.modelPicker.localGroupLabel"),
          children: {
            groupLabel: t("pages.assistant.modelPicker.localGroupLabel"),
            options: localModels.map(toOption),
          },
        })
      }
      if (cloudModels.length > 0) {
        groups.push({
          id: "cloud",
          label: t("pages.assistant.modelPicker.cloudGroupLabel"),
          children: {
            groupLabel: t("pages.assistant.modelPicker.cloudGroupLabel"),
            options: cloudModels.map(toOption),
          },
        })
      }
      return {
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
        options: groups,
        placeholder: t("common.fields.selectModel"),
        value: selectedModelId,
      }
    },
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
  // The ref is refreshed in an effect (after commit, before any click can fire)
  // — writing it during render trips the react-compiler refs lint.
  const onNewSessionRef = useRef(onNewSession)
  useEffect(() => {
    onNewSessionRef.current = onNewSession
  })
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
        className="size-8 shrink-0 rounded-full text-muted-foreground hover:bg-glass-bg-strong hover:text-foreground"
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
