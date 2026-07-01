import { act, renderHook, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useAgentSurfaceStore } from "@/store/useAgentSurfaceStore"

import { useAssistantAgent } from "../use-assistant-agent"

type MockSseMessage = {
  data: string
  id: string | null
}

type MockReadAssistantSseStreamOptions = {
  onMessage: (message: MockSseMessage) => void
  onOpen?: () => void
}

type MockReadAssistantSseStream = (
  url: string,
  options: MockReadAssistantSseStreamOptions
) => Promise<void>

const { mockMutateAsync, mockReadAssistantSseStream } = vi.hoisted(() => ({
  mockMutateAsync: vi.fn<() => Promise<unknown>>(),
  mockReadAssistantSseStream: vi.fn<MockReadAssistantSseStream>(),
}))

class MockWebSocket {
  static readonly CONNECTING = 0
  static readonly OPEN = 1
  static readonly CLOSING = 2
  static readonly CLOSED = 3

  readonly CONNECTING = MockWebSocket.CONNECTING
  readonly OPEN = MockWebSocket.OPEN
  readonly CLOSING = MockWebSocket.CLOSING
  readonly CLOSED = MockWebSocket.CLOSED

  readyState = MockWebSocket.CLOSED

  constructor(readonly url: string) {}

  addEventListener = vi.fn<() => void>()
  close = vi.fn<() => void>()
  removeEventListener = vi.fn<() => void>()
  send = vi.fn<() => void>()
}

vi.stubGlobal("WebSocket", MockWebSocket)

vi.mock("@slab/api", () => ({
  createSlabApiFetchClient: vi.fn<() => { POST: () => Promise<void> }>(() => ({
    POST: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  })),
  default: {
    useMutation: vi.fn<() => { isPending: boolean; mutateAsync: () => Promise<unknown> }>(() => ({
      isPending: false,
      mutateAsync: mockMutateAsync,
    })),
    useQuery: vi.fn<() => { data: { effective_value: boolean } }>(() => ({
      data: {
        effective_value: false,
      },
    })),
  },
  getErrorMessage: vi.fn<(error: unknown) => string>((error: unknown) =>
    error instanceof Error ? error.message : String(error)
  ),
}))

vi.mock("@slab/i18n", () => ({
  getResolvedAppLanguage: vi.fn<() => string>(() => "en-US"),
  translateServerField: vi.fn<(
    i18n: unknown,
    field: string,
    fallback: string,
    t?: unknown
  ) => string>((_i18n: unknown, _field: string, fallback: string) => fallback),
  useTranslation: vi.fn<() => { t: (key: string) => string }>(() => ({
    t: (key: string) => key,
  })),
}))

vi.mock("../../lib/assistant-sse", async () => {
  const actual = await vi.importActual<typeof import("../../lib/assistant-sse")>(
    "../../lib/assistant-sse"
  )

  return {
    ...actual,
    readAssistantSseStream: mockReadAssistantSseStream,
  }
})

function renderAssistantAgent() {
  return renderHook(() =>
    useAssistantAgent({
      model: "test-model",
      sessionId: "session-1",
    })
  )
}

describe("useAssistantAgent a2u tool dispatch", () => {
  let unmount: (() => void) | null = null

  beforeEach(() => {
    vi.clearAllMocks()
    unmount = null
    mockMutateAsync.mockResolvedValue({
      status: "running",
      thread_id: "thread-1",
      type: "agent.ack",
    })
    mockReadAssistantSseStream.mockResolvedValue(undefined)
    useAgentSurfaceStore.setState({
      draft: null,
      focusComposerSignal: 0,
      pendingSurface: null,
    })
  })

  afterEach(() => {
    unmount?.()
  })

  it("dispatches workspace.open tool calls to the agent surface store", async () => {
    mockReadAssistantSseStream.mockImplementation(async (_url, options) => {
      options.onOpen?.()
      options.onMessage({
        data: JSON.stringify({
          arguments: JSON.stringify({ path: "src/main.rs" }),
          call_id: "call-1",
          name: "workspace.open",
          sequence_number: 1,
          thread_id: "thread-1",
          type: "response.function_call_arguments.done",
        }),
        id: "1",
      })
    })

    const rendered = renderAssistantAgent()
    const { result } = rendered
    unmount = rendered.unmount

    await act(async () => {
      await result.current.handleSubmit("Open src/main.rs")
    })

    await waitFor(() => {
      expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
        payload: {
          revealPath: "src/main.rs",
        },
        type: "workspace",
      })
    })
  })

  it("dispatches plugin.launch tool calls as preview-only pending surfaces", async () => {
    mockReadAssistantSseStream.mockImplementation(async (_url, options) => {
      options.onOpen?.()
      options.onMessage({
        data: JSON.stringify({
          arguments: JSON.stringify({
            plugin_id: "demo-plugin",
            surface: "panel",
            payload: { taskId: "task-1" },
          }),
          call_id: "call-plugin",
          name: "plugin.launch",
          sequence_number: 1,
          thread_id: "thread-1",
          type: "response.function_call_arguments.done",
        }),
        id: "1",
      })
    })

    const rendered = renderAssistantAgent()
    const { result } = rendered
    unmount = rendered.unmount

    await act(async () => {
      await result.current.handleSubmit("Open plugin panel")
    })

    await waitFor(() => {
      expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
        payload: {
          pluginId: "demo-plugin",
          surface: "panel",
          payload: {
            taskId: "task-1",
          },
        },
        type: "plugin",
      })
    })
    expect(result.current.pendingApprovals).toEqual([])
  })

  it("keeps unknown tools on the ThoughtChain fallback path", async () => {
    mockReadAssistantSseStream.mockImplementation(async (_url, options) => {
      options.onOpen?.()
      options.onMessage({
        data: JSON.stringify({
          arguments: JSON.stringify({ path: "src/main.rs" }),
          call_id: "call-unknown",
          name: "read_file",
          sequence_number: 1,
          thread_id: "thread-1",
          type: "response.function_call_arguments.done",
        }),
        id: "1",
      })
    })

    const rendered = renderAssistantAgent()
    const { result } = rendered
    unmount = rendered.unmount

    await act(async () => {
      await result.current.handleSubmit("Read src/main.rs")
    })

    await waitFor(() => {
      expect(result.current.messages.at(-1)?.message.thoughts).toEqual([
        expect.objectContaining({
          callId: "call-unknown",
          status: "loading",
          toolName: "read_file",
        }),
      ])
    })
    expect(useAgentSurfaceStore.getState().pendingSurface).toBeNull()
  })

  it("stores turn_completed artifact refs on the final assistant message", async () => {
    mockReadAssistantSseStream.mockImplementation(async (_url, options) => {
      options.onOpen?.()
      options.onMessage({
        data: JSON.stringify({
          artifact_refs: [
            {
              kind: "file",
              path: "src/main.rs",
            },
          ],
          sequence_number: 1,
          text: "Opened the file.",
          thread_id: "thread-1",
          type: "response.output_text.done",
        }),
        id: "1",
      })
    })

    const rendered = renderAssistantAgent()
    const { result } = rendered
    unmount = rendered.unmount

    await act(async () => {
      await result.current.handleSubmit("Open src/main.rs")
    })

    await waitFor(() => {
      expect(result.current.messages.at(-1)).toMatchObject({
        message: {
          artifactRefs: [
            {
              kind: "file",
              path: "src/main.rs",
            },
          ],
          content: "Opened the file.",
          role: "assistant",
        },
        status: "success",
      })
    })
  })
})
