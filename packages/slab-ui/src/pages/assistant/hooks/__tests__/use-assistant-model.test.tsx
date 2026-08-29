import { renderHook } from "vitest-browser-react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const { useAiModelMock, toastMock } = vi.hoisted(() => ({
  useAiModelMock: vi.fn<() => unknown>(),
  toastMock: {
    success: vi.fn<(message: string) => void>(),
    error: vi.fn<(message: string, options?: { description?: string }) => void>(),
    message: vi.fn<(message: string) => void>(),
  },
}))

vi.mock("@slab/ui/hooks/use-ai-model", () => ({ useAiModel: useAiModelMock }))
vi.mock("sonner", () => ({ toast: toastMock }))
vi.mock("@slab/i18n", () => ({ default: { t: (key: string) => key }, useTranslation: () => ({ t: (key: string) => key }) }))
vi.mock("../../lib/assistant-request-errors", () => ({
  getAssistantErrorDescription: (error: unknown, fallback: string) =>
    error instanceof Error ? error.message : fallback,
}))

import type {
  AiModel,
  EnsureDownloadedResult,
  EnsureLoadedResult,
  UseAiModelResult,
} from "@slab/ui/hooks/use-ai-model"
import type { ModelRuntimeStatus } from "../../lib/assistant-page-state"
import { useAssistantModel } from "../use-assistant-model"

function aiModel(overrides: Partial<AiModel> = {}): AiModel {
  return {
    backend_id: null,
    backend_ids: [],
    capabilities: ["chat_generation"],
    chat_capabilities: null,
    created_at: "2026-01-01T00:00:00Z",
    display_name: "Model",
    filename: "model.gguf",
    id: "model-1",
    kind: "local",
    local_path: null,
    pending: false,
    repo_id: "owner/model",
    runtime_state: null,
    size_bytes: null,
    spec: {
      filename: "model.gguf",
      local_path: null,
      provider_id: null,
      remote_model_id: null,
      repo_id: "owner/model",
    },
    status: "ready",
    updated_at: "2026-01-01T00:00:00Z",
    ...overrides,
  }
}

function downloadResult(downloadedNow: boolean): EnsureDownloadedResult {
  return { model: aiModel(), modelPath: null, downloadedNow }
}

function loadResult(runtimeStatus: ModelRuntimeStatus | null): EnsureLoadedResult {
  return { model: aiModel(), modelPath: null, downloadedNow: false, loadedNow: false, runtimeStatus }
}

function aiModelResult(overrides: Partial<UseAiModelResult> = {}): UseAiModelResult {
  return {
    models: [],
    localModels: [],
    options: [],
    selectedId: "",
    setSelectedId: vi.fn<(value: string) => void>(),
    selected: undefined,
    loading: false,
    refetching: false,
    error: null,
    refetch: vi.fn<() => Promise<{ data: unknown }>>().mockResolvedValue({ data: {} }),
    status: { downloading: false, loading: false, switching: false, unloading: false, busy: false },
    download: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    ensureDownloaded: vi
      .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureDownloadedResult>>()
      .mockResolvedValue(downloadResult(false)),
    load: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    switchTo: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    unload: vi.fn<(modelId: string) => Promise<unknown>>().mockResolvedValue({}),
    ensureLoaded: vi
      .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureLoadedResult>>()
      .mockResolvedValue(loadResult(null)),
    ...overrides,
  }
}

function mockCatalog(result: UseAiModelResult) {
  useAiModelMock.mockReturnValue(result)
}

describe("useAssistantModel", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("maps catalog models to picker options and tracks the selected option", async () => {
    mockCatalog(
      aiModelResult({
        models: [
          aiModel({ kind: "cloud", display_name: "Cloud", id: "cloud-1" }),
          aiModel({ kind: "local", status: "ready", local_path: "/m.gguf", id: "local-1" }),
        ],
        selectedId: "local-1",
      }),
    )

    const { result } = await renderHook(() => useAssistantModel())

    const cloud = result.current.modelOptions.find((option) => option.id === "cloud-1")
    const local = result.current.modelOptions.find((option) => option.id === "local-1")
    expect(cloud).toMatchObject({ downloaded: true, source: "cloud", label: "Cloud" })
    expect(local).toMatchObject({ downloaded: true, source: "local" })
    expect(result.current.selectedModel?.id).toBe("local-1")
  })

  it("throws when preparing a model without a selection", async () => {
    mockCatalog(aiModelResult({ selectedId: "" }))
    const { result } = await renderHook(() => useAssistantModel())

    let thrown: unknown = null
    try {
      await result.current.ensureAssistantModelReady()
    } catch (error) {
      thrown = error
    }

    expect(thrown).toBeInstanceOf(Error)
    expect((thrown as Error).message).toBe("pages.assistant.error.selectModelFirst")
  })

  it("short-circuits cloud models without downloading", async () => {
    const result0 = aiModelResult({
      models: [aiModel({ kind: "cloud", id: "cloud-1" })],
      selectedId: "cloud-1",
    })
    mockCatalog(result0)
    const { result } = await renderHook(() => useAssistantModel())

    await result.current.ensureAssistantModelReady()

    expect(result0.ensureDownloaded).not.toHaveBeenCalled()
    expect(result.current.loadedModelStatus).toBeNull()
  })

  it("downloads, loads and surfaces runtime status for a local model", async () => {
    const result0 = aiModelResult({
      models: [aiModel({ kind: "local", status: "ready", local_path: "/m.gguf", id: "local-1" })],
      localModels: [aiModel({ kind: "local", status: "ready", local_path: "/m.gguf", id: "local-1" })],
      selectedId: "local-1",
      ensureDownloaded: vi
        .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureDownloadedResult>>()
        .mockResolvedValue(downloadResult(true)),
      ensureLoaded: vi
        .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureLoadedResult>>()
        .mockResolvedValue(loadResult({ context_length: 8192 } as ModelRuntimeStatus)),
    })
    mockCatalog(result0)
    const { result, act } = await renderHook(() => useAssistantModel())

    await act(async () => {
      await result.current.ensureAssistantModelReady()
    })

    expect(toastMock.success).toHaveBeenCalledWith("common.toasts.modelDownloaded")
    expect(result0.ensureLoaded).toHaveBeenCalledWith("local-1")
    expect(result.current.loadedModelStatus?.context_length).toBe(8192)
  })

  it("re-downloads with force and retries loading when the first load fails", async () => {
    const result0 = aiModelResult({
      models: [aiModel({ kind: "local", status: "ready", local_path: "/m.gguf", id: "local-1" })],
      localModels: [aiModel({ kind: "local", status: "ready", local_path: "/m.gguf", id: "local-1" })],
      selectedId: "local-1",
      ensureDownloaded: vi
        .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureDownloadedResult>>()
        .mockResolvedValue(downloadResult(false)),
      ensureLoaded: vi
        .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureLoadedResult>>()
        .mockRejectedValueOnce(new Error("load failed"))
        .mockResolvedValueOnce(loadResult(null)),
    })
    mockCatalog(result0)
    const { result } = await renderHook(() => useAssistantModel())

    await result.current.ensureAssistantModelReady()

    expect(toastMock.message).toHaveBeenCalledWith("pages.assistant.toast.modelLoadRetry")
    expect(result0.ensureDownloaded).toHaveBeenNthCalledWith(2, "local-1", { forceDownload: true })
    expect(result0.ensureLoaded).toHaveBeenCalledTimes(2)
  })

  it("rethrows a first load failure without retry when the model was just downloaded", async () => {
    const result0 = aiModelResult({
      models: [aiModel({ kind: "local", status: "ready", local_path: "/m.gguf", id: "local-1" })],
      localModels: [aiModel({ kind: "local", status: "ready", local_path: "/m.gguf", id: "local-1" })],
      selectedId: "local-1",
      ensureDownloaded: vi
        .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureDownloadedResult>>()
        .mockResolvedValue(downloadResult(true)),
      ensureLoaded: vi
        .fn<(modelId: string, options?: { forceDownload?: boolean }) => Promise<EnsureLoadedResult>>()
        .mockRejectedValueOnce(new Error("load failed")),
    })
    mockCatalog(result0)
    const { result } = await renderHook(() => useAssistantModel())

    let thrown: unknown = null
    try {
      await result.current.ensureAssistantModelReady()
    } catch (error) {
      thrown = error
    }

    expect(thrown).toBeInstanceOf(Error)
    expect((thrown as Error).message).toBe("load failed")
    expect(toastMock.message).not.toHaveBeenCalled()
    expect(toastMock.error).toHaveBeenCalledWith("pages.assistant.toast.failedToPrepareModel", {
      description: "load failed",
    })
  })
})
