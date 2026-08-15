import { renderHook } from "vitest-browser-react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const { toastMock } = vi.hoisted(() => ({
  toastMock: { info: vi.fn<(message: string) => void>() },
}))

vi.mock("sonner", () => ({ toast: toastMock }))
vi.mock("@slab/i18n", () => ({
  DEFAULT_ASSISTANT_LABELS: ["default.assistant.label"],
  LEGACY_DEFAULT_CHAT_LABELS: ["New Conversation"],
  useTranslation: () => ({ t: (key: string) => key }),
}))

import type { AssistantConversationItem } from "../use-assistant-sessions"
import { useAssistantConversationList } from "../use-assistant-conversation-list"

function item(overrides: Partial<AssistantConversationItem> = {}): AssistantConversationItem {
  return { key: "session-1", label: "First chat", group: "today", ...overrides }
}

type ListOptions = Parameters<typeof useAssistantConversationList>[0]

function baseOptions(overrides: Partial<ListOptions> = {}): ListOptions {
  return {
    conversationList: [item(), item({ key: "session-2", label: "Second chat" })],
    curConversation: "session-1",
    setCurConversation: vi.fn<(id: string) => void>(),
    deleteSession: vi.fn<(sessionId: string) => Promise<boolean>>().mockResolvedValue(true),
    updateSessionLabel: vi
      .fn<(sessionId: string, label: string) => Promise<boolean>>()
      .mockResolvedValue(true),
    isSessionBusy: false,
    isSessionBootstrapping: false,
    setIsSessionSheetOpen: vi.fn<(open: boolean) => void>(),
    ...overrides,
  }
}

describe("useAssistantConversationList", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("pins the current conversation to the front of the sorted list", async () => {
    const opts = baseOptions({ curConversation: "session-2" })
    const { result } = await renderHook(() => useAssistantConversationList(opts))

    expect(result.current.sortedConversations.map((conversation) => conversation.key)).toEqual([
      "session-2",
      "session-1",
    ])
  })

  it("falls back to the current session label when the conversation has none", async () => {
    const opts = baseOptions({
      conversationList: [item({ key: "session-1", label: "  " })],
    })
    const { result } = await renderHook(() => useAssistantConversationList(opts))

    expect(result.current.currentConversationLabel).toBe(
      "pages.assistant.sessionSummary.currentSession",
    )
  })

  it("renames a default-labeled conversation from the first prompt", async () => {
    const opts = baseOptions({
      conversationList: [item({ key: "session-1", label: "pages.assistant.runtime.newChat" })],
    })
    const { result } = await renderHook(() => useAssistantConversationList(opts))

    await result.current.setConversationLabelIfNeeded("session-1", "hello world")

    expect(opts.updateSessionLabel).toHaveBeenCalledWith("session-1", "hello world")
  })

  it("keeps a custom conversation label untouched", async () => {
    const opts = baseOptions({
      conversationList: [item({ key: "session-1", label: "Project review" })],
    })
    const { result } = await renderHook(() => useAssistantConversationList(opts))

    await result.current.setConversationLabelIfNeeded("session-1", "hello world")

    expect(opts.updateSessionLabel).not.toHaveBeenCalled()
  })

  it("blocks deletion while the session is busy and toasts", async () => {
    const opts = baseOptions({ isSessionBusy: true })
    const { result } = await renderHook(() => useAssistantConversationList(opts))

    void result.current.handleDeleteConversation("session-2")

    expect(opts.deleteSession).not.toHaveBeenCalled()
    expect(toastMock.info).toHaveBeenCalledWith("pages.assistant.toast.waitBeforeDeletingSessions")
  })

  it("deletes the conversation when the session is idle", async () => {
    const opts = baseOptions()
    const { result } = await renderHook(() => useAssistantConversationList(opts))

    await result.current.handleDeleteConversation("session-2")

    expect(opts.deleteSession).toHaveBeenCalledWith("session-2")
  })

  it("closes the sheet without switching when reselecting the current conversation", async () => {
    const opts = baseOptions({ curConversation: "session-1" })
    const { result } = await renderHook(() => useAssistantConversationList(opts))

    result.current.handleSelectConversation("session-1")

    expect(opts.setCurConversation).not.toHaveBeenCalled()
    expect(opts.setIsSessionSheetOpen).toHaveBeenCalledWith(false)
  })

  it("blocks switching while the session is bootstrapping and toasts", async () => {
    const opts = baseOptions({ isSessionBootstrapping: true })
    const { result } = await renderHook(() => useAssistantConversationList(opts))

    result.current.handleSelectConversation("session-2")

    expect(opts.setCurConversation).not.toHaveBeenCalled()
    expect(toastMock.info).toHaveBeenCalledWith("pages.assistant.toast.sessionSyncing")
  })

  it("switches the current conversation and closes the sheet when idle", async () => {
    const opts = baseOptions({ curConversation: "session-1" })
    const { result } = await renderHook(() => useAssistantConversationList(opts))

    result.current.handleSelectConversation("session-2")

    expect(opts.setCurConversation).toHaveBeenCalledWith("session-2")
    expect(opts.setIsSessionSheetOpen).toHaveBeenCalledWith(false)
  })
})
