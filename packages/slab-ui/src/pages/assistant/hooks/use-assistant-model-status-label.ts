import { useMemo } from "react"

import { getResolvedAppLanguage, useTranslation } from "@slab/i18n"

import {
  getSelectedModelStatusLabel,
  type ModelOption,
  type ModelRuntimeStatus,
} from "../lib/assistant-page-state"

type UseAssistantModelStatusLabelOptions = {
  curConversation: string | undefined
  isCreatingSession: boolean
  isDeletingSession: boolean
  isHistoryLoading: boolean
  restoredThreadId: string | null
  isPreparingModel: boolean
  isSessionBootstrapping: boolean
  modelLoading: boolean
  selectedModel: ModelOption | undefined
  loadedModelStatus: ModelRuntimeStatus | null
}

export function useAssistantModelStatusLabel({
  curConversation,
  isCreatingSession,
  isDeletingSession,
  isHistoryLoading,
  restoredThreadId,
  isPreparingModel,
  isSessionBootstrapping,
  modelLoading,
  selectedModel,
  loadedModelStatus,
}: UseAssistantModelStatusLabelOptions) {
  const { t } = useTranslation()
  const resolvedLanguage = getResolvedAppLanguage()

  const statusLabel = useMemo(
    () =>
      getSelectedModelStatusLabel({
        curConversation,
        eventsConnected: Boolean(restoredThreadId) || !isHistoryLoading,
        isCreatingSession,
        isDeletingSession,
        isHistoryLoading,
        isPreparingModel,
        isSessionBootstrapping,
        modelLoading,
        resolvedLanguage,
        selectedModel,
        selectedRuntimeContextLength: loadedModelStatus?.context_length ?? null,
        t,
      }),
    [
      curConversation,
      isCreatingSession,
      isDeletingSession,
      isHistoryLoading,
      isPreparingModel,
      isSessionBootstrapping,
      loadedModelStatus,
      modelLoading,
      resolvedLanguage,
      restoredThreadId,
      selectedModel,
      t,
    ]
  )

  return { statusLabel }
}
