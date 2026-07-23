import { renderHook } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

vi.mock("@slab/i18n", () => ({
  getResolvedAppLanguage: () => "en-US",
  useTranslation: () => ({ t: (key: string) => key }),
}))

const { labelSpy } = vi.hoisted(() => ({
  labelSpy: vi.fn<(options: Record<string, unknown>) => string>(
    (options) =>
      `events:${options.eventsConnected}:ctx:${options.selectedRuntimeContextLength ?? "none"}:lang:${options.resolvedLanguage}`,
  ),
}))

vi.mock("../../lib/assistant-page-state", () => ({
  getSelectedModelStatusLabel: labelSpy,
}))

import type { ModelOption, ModelRuntimeStatus } from "../../lib/assistant-page-state"
import { useAssistantModelStatusLabel } from "../use-assistant-model-status-label"

function selectedModel(overrides: Partial<ModelOption> = {}): ModelOption {
  return {
    capabilities: { raw_gbnf: true, reasoning_controls: false, structured_output: true },
    downloaded: true,
    id: "model-1",
    label: "Local Model",
    pending: false,
    source: "local",
    ...overrides,
  }
}

const base = {
  curConversation: "session-1" as string | undefined,
  isCreatingSession: false,
  isDeletingSession: false,
  isHistoryLoading: false,
  restoredThreadId: null,
  isPreparingModel: false,
  isSessionBootstrapping: false,
  modelLoading: false,
  selectedModel: selectedModel(),
  loadedModelStatus: null as ModelRuntimeStatus | null,
}

describe("useAssistantModelStatusLabel", () => {
  it("wires the resolved language and selected model into the label helper", () => {
    const { result } = renderHook(() => useAssistantModelStatusLabel(base))

    expect(labelSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        resolvedLanguage: "en-US",
        selectedModel: selectedModel(),
        selectedRuntimeContextLength: null,
      }),
    )
    expect(result.current.statusLabel).toBe("events:true:ctx:none:lang:en-US")
  })

  it("maps loaded model status context length to the runtime context window", () => {
    renderHook(() =>
      useAssistantModelStatusLabel({
        ...base,
        loadedModelStatus: { context_length: 8192 } as ModelRuntimeStatus,
      }),
    )

    expect(labelSpy).toHaveBeenCalledWith(
      expect.objectContaining({ selectedRuntimeContextLength: 8192 }),
    )
  })

  it("treats a restored thread as connected even while history is still loading", () => {
    renderHook(() =>
      useAssistantModelStatusLabel({
        ...base,
        restoredThreadId: "thread-1",
        isHistoryLoading: true,
      }),
    )

    expect(labelSpy).toHaveBeenCalledWith(expect.objectContaining({ eventsConnected: true }))
  })

  it("reports disconnected while history loads without a restored thread", () => {
    renderHook(() => useAssistantModelStatusLabel({ ...base, isHistoryLoading: true }))

    expect(labelSpy).toHaveBeenCalledWith(expect.objectContaining({ eventsConnected: false }))
  })
})
