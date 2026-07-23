import type { components } from "@slab/api"
import type { AiModel } from "@/hooks/use-ai-model"

type AssistantPageTranslation = (key: string, values?: Record<string, unknown>) => string

type ModelOptionSource = "local" | "cloud"

type AssistantModelCapabilities = {
  raw_gbnf: boolean
  structured_output: boolean
  reasoning_controls: boolean
}

export type ModelOption = {
  id: string
  label: string
  downloaded: boolean
  pending: boolean
  source: ModelOptionSource
  capabilities: AssistantModelCapabilities
  contextWindow?: number | null
  runtimePresets?: AiModel["runtime_presets"]
}

export type ModelRuntimeStatus = components["schemas"]["ModelStatusResponse"]

export function createConversationLabel(value: string, fallback: string) {
  const trimmed = value.trim()

  if (!trimmed) {
    return fallback
  }

  return trimmed.length > 42 ? `${trimmed.slice(0, 42)}...` : trimmed
}

function defaultCapabilitiesForSource(source: ModelOptionSource): AssistantModelCapabilities {
  return source === "cloud"
    ? {
      raw_gbnf: false,
      reasoning_controls: true,
      structured_output: true,
    }
    : {
      raw_gbnf: true,
      reasoning_controls: false,
      structured_output: true,
    }
}

export function resolveAssistantModelCapabilities(
  model: Pick<AiModel, "chat_capabilities" | "kind">
): AssistantModelCapabilities {
  return model.chat_capabilities ?? defaultCapabilitiesForSource(model.kind)
}

export function toAssistantModelOption(model: AiModel): ModelOption {
  return {
    capabilities: resolveAssistantModelCapabilities(model),
    contextWindow: model.spec.context_window ?? null,
    downloaded:
      model.kind === "cloud" ||
      (model.status === "ready" &&
        typeof model.local_path === "string" &&
        model.local_path.length > 0),
    id: model.id,
    label: model.display_name,
    pending: model.pending,
    runtimePresets: model.runtime_presets ?? null,
    source: model.kind,
  }
}

export function getGreeting(date: Date, t: AssistantPageTranslation) {
  const hour = date.getHours()

  if (hour < 12) {
    return t("pages.assistant.greeting.morning")
  }

  if (hour < 18) {
    return t("pages.assistant.greeting.afternoon")
  }

  return t("pages.assistant.greeting.evening")
}

type SelectedModelStatusLabelOptions = {
  curConversation?: string | null
  eventsConnected: boolean
  isCreatingSession: boolean
  isDeletingSession: boolean
  isHistoryLoading: boolean
  isPreparingModel: boolean
  isSessionBootstrapping: boolean
  modelLoading: boolean
  resolvedLanguage: string
  selectedModel?: ModelOption
  selectedRuntimeContextLength?: number | null
  t: AssistantPageTranslation
}

export function getSelectedModelStatusLabel({
  curConversation,
  eventsConnected,
  isCreatingSession,
  isDeletingSession,
  isHistoryLoading,
  isPreparingModel,
  isSessionBootstrapping,
  modelLoading,
  resolvedLanguage,
  selectedModel,
  selectedRuntimeContextLength,
  t,
}: SelectedModelStatusLabelOptions) {
  if (isSessionBootstrapping || !curConversation) {
    return t("pages.assistant.status.preparingSession")
  }

  if (isHistoryLoading) {
    return t("pages.assistant.status.loadingSessionHistory")
  }

  if (isCreatingSession) {
    return t("pages.assistant.status.creatingSession")
  }

  if (isDeletingSession) {
    return t("pages.assistant.status.deletingSession")
  }

  if (modelLoading) {
    return t("pages.assistant.status.loadingModels")
  }

  if (!selectedModel) {
    return t("pages.assistant.status.selectModel")
  }

  const parts = [selectedModel.label]

  if (selectedRuntimeContextLength && selectedRuntimeContextLength > 0) {
    parts.push(
      t("pages.assistant.status.runtimeContextWindow", {
        formatted: new Intl.NumberFormat(resolvedLanguage).format(selectedRuntimeContextLength),
      })
    )
  } else if (selectedModel.contextWindow && selectedModel.contextWindow > 0) {
    parts.push(
      t("pages.assistant.status.contextWindow", {
        formatted: new Intl.NumberFormat(resolvedLanguage).format(selectedModel.contextWindow),
      })
    )
  } else if (selectedModel.pending) {
    parts.push(t("pages.assistant.status.downloading"))
  } else if (selectedModel.source === "local" && !selectedModel.downloaded) {
    parts.push(t("pages.assistant.status.needsDownload"))
  } else if (isPreparingModel) {
    parts.push(t("pages.assistant.status.preparing"))
  } else if (selectedModel.source === "cloud") {
    parts.push(t("pages.assistant.status.cloudModel"))
  }

  if (eventsConnected) {
    parts.push(t("pages.assistant.connection.connected"))
  }

  return parts.join(" / ")
}
