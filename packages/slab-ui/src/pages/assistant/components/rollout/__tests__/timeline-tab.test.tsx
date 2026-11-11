/**
 * Regression: the timeline reuses the assistant `MessageItem`, which wraps
 * every row in `MessageScrollerItem` — that primitive throws outside a
 * `MessageScroller` context. The real `@slab/components/message-scroller` is
 * deliberately NOT mocked here so the provider wiring stays under test.
 */

import type { ReactNode } from "react"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

vi.mock("@slab/i18n", async () => {
  const { setupSlabI18nMock } = await import("@slab/test-utils/mocks")
  return setupSlabI18nMock()
})

vi.mock("@slab/api", () => ({
  default: {
    useQuery: () => ({
      data: {
        thread: {
          turns: [
            {
              items: [
                { id: "u1", type: "userMessage", content: [{ type: "text", text: "hello" }] },
                { id: "a1", type: "agentMessage", text: "world" },
              ],
            },
          ],
        },
        turn_prompts: [],
      },
      error: null,
      isLoading: false,
    }),
  },
}))

vi.mock("motion/react", () => ({
  motion: { create: <C,>(component: C): C => component },
  useReducedMotion: () => null,
}))

vi.mock("@mantine/hooks", () => ({
  useClipboard: () => ({ copy: vi.fn<() => void>(), copied: false }),
}))

vi.mock("@slab/ui/pages/assistant/lib/message-animations", () => ({
  MESSAGE_ANIMATIONS: { "slide-up": { variants: {} } },
}))

vi.mock("@slab/components/message", () => ({
  Message: ({ children, ...rest }: { children: ReactNode } & Record<string, unknown>) => (
    <div {...rest}>{children}</div>
  ),
  MessageContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageAvatar: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  MessageFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/bubble", () => ({
  Bubble: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  BubbleContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}))

vi.mock("@slab/components/button", () => ({
  Button: ({
    children,
    ...rest
  }: {
    children: ReactNode
  } & Record<string, unknown>) => (
    <button type="button" {...rest}>
      {children}
    </button>
  ),
}))

import { TimelineTab } from "../timeline-tab"

describe("TimelineTab", () => {
  it("renders message rows without a MessageScroller crash", async () => {
    const screen = await render(<TimelineTab threadId="thread-1" />)

    // The user/assistant rows come from `MessageItem`, i.e. from inside real
    // `MessageScrollerItem`s — this throws if the scroller context is missing.
    await expect
      .element(screen.getByTestId("assistant-message-user"))
      .toBeVisible()
    await expect
      .element(screen.getByTestId("assistant-message-assistant"))
      .toBeVisible()
  })
})
