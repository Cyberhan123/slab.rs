import { useMemo } from "react"

import { useTranslation } from "@slab/i18n"
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

  useHeader({ history: headerHistoryButton, select: headerModelPicker })
}
