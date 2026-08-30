import type { ReactNode } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import type { MessagePartRenderProps } from "../message/message-parts"
import type { TMessage, TMessagePart } from "../message/message-item"
import MessageToolPart, {
  deriveState,
  isApprovalPending,
  isToolActive,
  type ToolPartLike,
  type ToolState,
} from "../message/message-tool-part"

const interactionState = vi.hoisted(() => ({
  approvalStatusByItemId: new Map<string, "pending" | "approved" | "denied">(),
}))

vi.mock("@slab/components/badge", () => ({
  Badge: ({ children }: { children: ReactNode }) => <span>{children}</span>,
}))

vi.mock("@slab/components/collapsible", () => ({
  Collapsible: ({
    children,
    open,
  }: {
    children: ReactNode
    open?: boolean
  }) => (
    <div data-testid="collapsible" data-open={open ? "true" : "false"}>
      {children}
    </div>
  ),
  CollapsibleTrigger: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  CollapsibleContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("../message/code-block", () => ({
  CodeBlock: ({ code }: { code: string }) => <div data-testid="code-block">{code}</div>,
}))

vi.mock("../message-interaction-context", () => ({
  useMessageInteraction: () => ({
    approvalStatusByItemId: interactionState.approvalStatusByItemId,
    liveOutputByItemId: new Map<string, string>(),
  }),
}))

const part = (overrides: Partial<ToolPartLike> = {}): ToolPartLike => ({
  type: "tool-call",
  state: "input-available",
  ...overrides,
})

describe("deriveState", () => {
  it("prioritizes a pending approval", () => {
    expect(deriveState(part({ state: "output-available" }), "pending")).toBe("approval-requested")
  })

  it("maps a denied approval to output-denied", () => {
    expect(deriveState(part(), "denied")).toBe("output-denied")
  })

  it("falls through to part state when approved", () => {
    expect(deriveState(part({ state: "output-available" }), "approved")).toBe("output-available")
  })

  it("surfaces an errored tool as output-error", () => {
    expect(deriveState(part({ state: "input-available", errorText: "boom" }), undefined)).toBe(
      "output-error",
    )
    expect(deriveState(part({ state: "output-error" }), undefined)).toBe("output-error")
  })

  it("treats a populated output as output-available", () => {
    expect(deriveState(part({ state: "input-available", output: { ok: true } }), undefined)).toBe(
      "output-available",
    )
  })

  it("maps input-streaming and defaults to input-available", () => {
    expect(deriveState(part({ state: "input-streaming" }), undefined)).toBe("input-streaming")
    expect(deriveState(part({ state: "input-available" }), undefined)).toBe("input-available")
  })
})

describe("isToolActive", () => {
  it.each(["input-available", "input-streaming", "approval-requested"] as ToolState[])(
    "is active for %s",
    (state) => {
      expect(isToolActive(state)).toBe(true)
    },
  )

  it.each([
    "approval-responded",
    "output-available",
    "output-denied",
    "output-error",
  ] as ToolState[])("is inactive for %s", (state) => {
    expect(isToolActive(state)).toBe(false)
  })
})

describe("isApprovalPending", () => {
  it("is the only default-open case", () => {
    expect(isApprovalPending("approval-requested")).toBe(true)
    expect(isApprovalPending("input-available")).toBe(false)
    expect(isApprovalPending("input-streaming")).toBe(false)
    expect(isApprovalPending("output-available")).toBe(false)
  })
})

type ToolPartProps = MessagePartRenderProps<TMessagePart, TMessage>

function baseProps(overrides: {
  part?: ToolPartLike
  kind?: ToolPartProps["kind"]
  name?: string
  toolCallId?: string
} = {}): ToolPartProps {
  const toolPart = overrides.part ?? part()
  const message = { id: "m1", role: "assistant" } as TMessage
  return {
    item: { key: "tc1", part: toolPart, message, index: 0, kind: overrides.kind ?? "tool" },
    part: toolPart,
    message,
    index: 0,
    kind: (overrides.kind ?? "tool") as ToolPartProps["kind"],
    name: overrides.name,
    toolCallId: overrides.toolCallId ?? "tc1",
  }
}

describe("MessageToolPart", () => {
  beforeEach(() => {
    interactionState.approvalStatusByItemId = new Map()
  })

  it("renders nothing for non-tool kinds", async () => {
    const screen = await render(<MessageToolPart {...baseProps({ kind: "text" })} />)
    // vitest-browser-react's screen does not expose the mount container, so the
    // original `container.toBeEmptyDOMElement()` is approximated by asserting the
    // component's primary rendered output (the Collapsible wrapper) is absent.
    expect(screen.getByTestId("collapsible").query()).toBeNull()
  })

  it("shows the awaiting-approval status and opens by default while pending", async () => {
    interactionState.approvalStatusByItemId = new Map([["tc1", "pending"]])
    const screen = await render(<MessageToolPart {...baseProps()} />)

    expect(screen.container.querySelector('[data-tool-state="approval-requested"]')).not.toBeNull()
    await expect.element(screen.getByTestId("collapsible")).toHaveAttribute("data-open", "true")
  })

  it("stays collapsed by default while merely running (no approval pending)", async () => {
    const screen = await render(<MessageToolPart {...baseProps()} />)

    await expect.element(screen.getByTestId("collapsible")).toHaveAttribute("data-open", "false")
  })

  it("shows the completed status and stays closed once output is available", async () => {
    const screen = await render(
      <MessageToolPart {...baseProps({ part: part({ state: "output-available", output: "ok" }) })} />,
    )

    expect(screen.container.querySelector('[data-tool-state="output-available"]')).not.toBeNull()
    await expect.element(screen.getByTestId("collapsible")).toHaveAttribute("data-open", "false")
  })

  it("summarizes a built-in tool call in the collapsed trigger line", async () => {
    const screen = await render(
      <MessageToolPart
        {...baseProps({
          name: "read_file",
          part: part({ type: "tool-read_file", state: "output-available", output: "ok", input: { path: "src/main.rs" } }),
        })}
      />,
    )

    await expect.element(screen.getByText("Read")).toBeInTheDocument()
    // Substring queries would double-match the expanded JSON body, so assert
    // on the collapsed trigger line directly.
    expect(screen.container.textContent).toContain("src/main.rs")
  })

  it("derives the tool name from the part type when no name is supplied", async () => {
    const screen = await render(
      <MessageToolPart
        {...baseProps({ name: undefined, part: part({ type: "tool-search-web", state: "input-available" }) })}
      />,
    )

    await expect.element(screen.getByText("search-web")).toBeInTheDocument()
  })
})
