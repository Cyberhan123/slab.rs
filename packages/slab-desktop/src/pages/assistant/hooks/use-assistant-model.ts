import { useCallback, useMemo, useState } from "react"
import { toast } from "sonner"

import { useTranslation } from "@slab/i18n"
import { useAiModel } from "@/hooks/use-ai-model"
import { HEADER_SELECT_KEYS } from "@/layouts/header"

import { getAssistantErrorDescription } from "../lib/assistant-request-errors"
import {
  resolveAssistantModelCapabilities,
  type ModelOption,
  type ModelRuntimeStatus,
} from "../lib/assistant-page-state"

export function useAssistantModel() {
  const { t } = useTranslation()
  const [loadedModelStatus, setLoadedModelStatus] = useState<ModelRuntimeStatus | null>(null)

  const assistantModels = useAiModel({
    capability: "chat_generation",
    storageKey: HEADER_SELECT_KEYS.assistantModel,
    includeCloud: true,
  })

  const modelOptions = useMemo<ModelOption[]>(
    () =>
      assistantModels.models.map((model) => {
        const downloaded =
          model.kind === "cloud" ||
          (model.status === "ready" &&
            typeof model.local_path === "string" &&
            model.local_path.length > 0)

        return {
          capabilities: resolveAssistantModelCapabilities(model),
          contextWindow: model.spec.context_window ?? null,
          downloaded,
          id: model.id,
          label: model.display_name,
          pending: model.pending,
          runtimePresets: model.runtime_presets ?? null,
          source: model.kind,
        }
      }),
    [assistantModels.models]
  )

  const selectedModelId = assistantModels.selectedId
  const setSelectedModelId = assistantModels.setSelectedId
  const selectedModel = useMemo(
    () => modelOptions.find((item) => item.id === selectedModelId),
    [modelOptions, selectedModelId]
  )

  const prepareSelectedModel = useCallback(async () => {
    if (!selectedModelId) {
      throw new Error(t("pages.assistant.error.selectModelFirst"))
    }

    const selectedOption = modelOptions.find((item) => item.id === selectedModelId)
    if (!selectedOption) {
      throw new Error(t("pages.assistant.error.selectedModelUnavailable"))
    }

    if (selectedOption.source === "cloud") {
      setLoadedModelStatus(null)
      return
    }

    const selectedLocal = assistantModels.localModels.find((item) => item.id === selectedModelId)
    const { downloadedNow } = await assistantModels.ensureDownloaded(selectedModelId)

    if (downloadedNow) {
      toast.success(
        t("pages.assistant.toast.downloaded", {
          model: selectedLocal?.display_name ?? selectedModelId,
        })
      )
    }

    try {
      const status = await assistantModels.ensureLoaded(selectedModelId)
      if (status.runtimeStatus) {
        setLoadedModelStatus(status.runtimeStatus)
      }
    } catch (firstLoadError) {
      if (downloadedNow) {
        throw firstLoadError
      }

      toast.message(t("pages.assistant.toast.modelLoadRetry"))

      const retry = await assistantModels.ensureDownloaded(selectedModelId, { forceDownload: true })
      if (retry.downloadedNow) {
        toast.success(
          t("pages.assistant.toast.downloaded", {
            model: selectedLocal?.display_name ?? selectedModelId,
          })
        )
      }

      const status = await assistantModels.ensureLoaded(selectedModelId)
      if (status.runtimeStatus) {
        setLoadedModelStatus(status.runtimeStatus)
      }
    }
  }, [assistantModels, modelOptions, selectedModelId, t])

  const ensureAssistantModelReady = useCallback(async () => {
    try {
      await prepareSelectedModel()
    } catch (error) {
      toast.error(t("pages.assistant.toast.failedToPrepareModel"), {
        description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError"), t),
      })
      throw error
    }
  }, [prepareSelectedModel, t])

  const modelLoading = assistantModels.loading
  const isPreparingModel = assistantModels.status.busy

  return {
    modelOptions,
    selectedModel,
    selectedModelId,
    setSelectedModelId,
    modelLoading,
    isPreparingModel,
    ensureAssistantModelReady,
    loadedModelStatus,
  }
}
