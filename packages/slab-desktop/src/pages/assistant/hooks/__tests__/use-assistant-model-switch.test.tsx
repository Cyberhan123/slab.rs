import { act, renderHook } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const { toastMock } = vi.hoisted(() => ({
  toastMock: { info: vi.fn<(message: string) => void>() },
}))

vi.mock("sonner", () => ({ toast: toastMock }))
vi.mock("@slab/i18n", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

import type { ModelOption } from "../../lib/assistant-page-state"
import { useAssistantModelSwitch } from "../use-assistant-model-switch"

function modelOption(overrides: Partial<ModelOption> = {}): ModelOption {
  return {
    capabilities: { raw_gbnf: true, reasoning_controls: false, structured_output: true },
    downloaded: true,
    id: "model-a",
    label: "Model A",
    pending: false,
    source: "local",
    ...overrides,
  }
}

type SwitchOptions = Parameters<typeof useAssistantModelSwitch>[0]

function baseOptions(overrides: Partial<SwitchOptions> = {}): SwitchOptions {
  return {
    modelOptions: [modelOption(), modelOption({ id: "model-b", label: "Model B" })],
    selectedModelId: "model-a",
    setSelectedModelId: vi.fn<(value: string) => void>(),
    isSessionBusy: false,
    isSessionBootstrapping: false,
    curConversation: "session-1",
    messageCount: 5,
    createSession: vi
      .fn<(options?: { select?: boolean; quiet?: boolean }) => Promise<{ id: string } | null>>()
      .mockResolvedValue({ id: "session-2" }),
    isCreatingSession: false,
    ...overrides,
  }
}

describe("useAssistantModelSwitch", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("ignores picker changes that match the current selection", () => {
    const opts = baseOptions()
    const { result } = renderHook(() => useAssistantModelSwitch(opts))

    act(() => result.current.handleModelPickerChange("model-a"))

    expect(opts.setSelectedModelId).not.toHaveBeenCalled()
    expect(result.current.pendingModelSwitchId).toBeNull()
  })

  it("blocks switching while the session is busy and toasts", () => {
    const opts = baseOptions({ isSessionBusy: true })
    const { result } = renderHook(() => useAssistantModelSwitch(opts))

    act(() => result.current.handleModelPickerChange("model-b"))

    expect(opts.setSelectedModelId).not.toHaveBeenCalled()
    expect(result.current.pendingModelSwitchId).toBeNull()
    expect(toastMock.info).toHaveBeenCalledWith("pages.assistant.toast.waitBeforeSwitchingModels")
  })

  it("switches immediately when there is no active conversation", () => {
    const opts = baseOptions({ curConversation: undefined, messageCount: 0 })
    const { result } = renderHook(() => useAssistantModelSwitch(opts))

    act(() => result.current.handleModelPickerChange("model-b"))

    expect(opts.setSelectedModelId).toHaveBeenCalledWith("model-b")
    expect(result.current.pendingModelSwitchId).toBeNull()
  })

  it("opens a pending switch confirmation when the session has messages", () => {
    const opts = baseOptions()
    const { result } = renderHook(() => useAssistantModelSwitch(opts))

    act(() => result.current.handleModelPickerChange("model-b"))

    expect(opts.setSelectedModelId).not.toHaveBeenCalled()
    expect(result.current.pendingModelSwitchId).toBe("model-b")
    expect(result.current.pendingModelSwitch?.id).toBe("model-b")
  })

  it("keeps the current session and applies the pending model", () => {
    const opts = baseOptions()
    const { result } = renderHook(() => useAssistantModelSwitch(opts))

    act(() => result.current.handleModelPickerChange("model-b"))
    act(() => result.current.handleKeepSessionOnModelSwitch())

    expect(opts.setSelectedModelId).toHaveBeenCalledWith("model-b")
    expect(result.current.pendingModelSwitchId).toBeNull()
  })

  it("creates a new session and applies the pending model on confirm", async () => {
    const opts = baseOptions()
    const { result } = renderHook(() => useAssistantModelSwitch(opts))

    act(() => result.current.handleModelPickerChange("model-b"))
    await act(async () => {
      await result.current.handleCreateSessionOnModelSwitch()
    })

    expect(opts.createSession).toHaveBeenCalledWith({ select: true })
    expect(opts.setSelectedModelId).toHaveBeenCalledWith("model-b")
    expect(result.current.pendingModelSwitchId).toBeNull()
  })

  it("leaves the pending switch intact when session creation returns null", async () => {
    const opts = baseOptions({
      createSession: vi
        .fn<(options?: { select?: boolean; quiet?: boolean }) => Promise<{ id: string } | null>>()
        .mockResolvedValue(null),
    })
    const { result } = renderHook(() => useAssistantModelSwitch(opts))

    act(() => result.current.handleModelPickerChange("model-b"))
    await act(async () => {
      await result.current.handleCreateSessionOnModelSwitch()
    })

    expect(opts.setSelectedModelId).not.toHaveBeenCalled()
    expect(result.current.pendingModelSwitchId).toBe("model-b")
  })

  it("does not close the pending switch while a session is being created", () => {
    const opts = baseOptions({ isCreatingSession: true })
    const { result } = renderHook(() => useAssistantModelSwitch(opts))

    act(() => result.current.handleModelPickerChange("model-b"))
    act(() => result.current.closePendingModelSwitch())

    expect(result.current.pendingModelSwitchId).toBe("model-b")
  })
})
