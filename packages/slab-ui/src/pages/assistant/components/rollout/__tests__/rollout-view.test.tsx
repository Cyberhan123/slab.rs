import { beforeEach, describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import { RolloutView } from "../rollout-view"

const { useQueryMock } = vi.hoisted(() => ({
  useQueryMock: vi.fn<(method: string, path: string) => unknown>(),
}))

vi.mock("@slab/api", () => ({
  // `useGuardrailFlag` and the rollout tabs share the mocked react-query
  // wrapper; each test dispatches on the endpoint path.
  default: { useQuery: useQueryMock },
}))

vi.mock("@slab/i18n", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

// The tab bodies each pull the whole assistant message renderer / JSON blocks;
// the gating and thread binding under test live in RolloutView itself.
vi.mock("../timeline-tab", () => ({
  TimelineTab: ({ threadId }: { threadId: string }) => (
    <div data-testid="timeline-tab">{threadId}</div>
  ),
}))
vi.mock("../lines-tab", () => ({
  LinesTab: ({ threadId }: { threadId: string }) => (
    <div data-testid="lines-tab">{threadId}</div>
  ),
}))
vi.mock("../trace-tab", () => ({
  TraceTab: ({ threadId, hasTrace }: { threadId: string; hasTrace?: boolean }) => (
    <div data-testid="trace-tab">
      {threadId}
      {hasTrace === undefined ? null : String(hasTrace)}
    </div>
  ),
}))

function mockApi({ debugEnabled }: { debugEnabled: boolean }) {
  useQueryMock.mockImplementation((_method: string, path: string) => {
    if (path === "/v1/settings/{pmid}") {
      return { data: { effective_value: debugEnabled } }
    }
    return { data: undefined, isLoading: false, error: null }
  })
}

describe("RolloutView", () => {
  beforeEach(() => {
    useQueryMock.mockReset()
  })

  it("stays gated behind the agent.debug setting", async () => {
    mockApi({ debugEnabled: false })
    await render(<RolloutView threadId="th-1" />)
    expect(document.body.textContent).toContain("pages.agentRollouts.disabled.title")
    // No rollout endpoint was touched — the per-thread tab queries never fire
    // (the disabled sessions-list hook may still register its (unused) call).
    const queriedPaths = useQueryMock.mock.calls.map((call) => call[1])
    expect(queriedPaths).not.toContain("/v1/agents/rollouts/{thread_id}/timeline")
    expect(queriedPaths).not.toContain("/v1/agents/rollouts/{thread_id}/lines")
    expect(queriedPaths).not.toContain("/v1/agents/rollouts/{thread_id}/trace")
  })

  it("asks for a conversation before a thread exists", async () => {
    mockApi({ debugEnabled: true })
    await render(<RolloutView threadId={null} />)
    expect(document.body.textContent).toContain("pages.agentRollouts.noThread")
  })

  it("binds the debug tabs to the current thread", async () => {
    mockApi({ debugEnabled: true })
    const screen = await render(<RolloutView threadId="th-42" />)
    const text = document.body.textContent ?? ""
    expect(text).toContain("pages.agentRollouts.tabs.timeline")
    expect(text).toContain("pages.agentRollouts.tabs.lines")
    expect(text).toContain("pages.agentRollouts.tabs.trace")
    expect(text).toContain("th-42")
    // The default (timeline) tab renders bound to the thread.
    await expect.element(screen.getByTestId("timeline-tab")).toHaveTextContent("th-42")
  })
})
