import { userEvent } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import Sender from "../sender"
import type { CommandInfo } from "../../lib/harness"

/** Mirror of the server-side `command/list` snapshot: the three built-ins. */
const COMMANDS: CommandInfo[] = [
  {
    name: "compact",
    aliases: [],
    description: "Summarize the conversation history to reclaim context.",
    kind: "control",
    source: "builtin",
    controlAction: "compact",
  },
  {
    name: "fork",
    aliases: [],
    description: "Branch the current thread into a new child thread.",
    kind: "control",
    source: "builtin",
    controlAction: "fork",
  },
  {
    name: "plan",
    aliases: [],
    description: "Seed a planning prompt for the model.",
    kind: "prompt",
    source: "builtin",
  },
  {
    name: "summarize",
    aliases: [],
    description: "Summarize the conversation.",
    kind: "prompt",
    source: "skill",
  },
]

describe("Sender", () => {
  it("submits trimmed text and clears the textarea", async () => {
    const onSubmit = vi.fn()

    const screen = await render(<Sender onSubmit={onSubmit} commands={COMMANDS} interactionMode="default" onInteractionModeChange={vi.fn()} />)

    const textarea = screen.getByLabelText("Message")
    await userEvent.type(textarea, "  hello slab  ")
    await userEvent.click(screen.getByRole("button", { name: "Send" }))

    expect(onSubmit).toHaveBeenCalledWith(
      "hello slab",
      expect.objectContaining({ files: [], effort: "off" }),
      expect.anything(),
    )
    await expect.element(textarea).toHaveValue("")
  })

  it("does not submit empty text", async () => {
    const onSubmit = vi.fn()

    const screen = await render(<Sender onSubmit={onSubmit} commands={COMMANDS} interactionMode="default" onInteractionModeChange={vi.fn()} />)

    await userEvent.type(screen.getByLabelText("Message"), "   ")
    await expect.element(screen.getByRole("button", { name: "Send" })).toBeDisabled()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it("disables input while loading", async () => {
    const screen = await render(<Sender loading onSubmit={vi.fn()} commands={COMMANDS} interactionMode="default" onInteractionModeChange={vi.fn()} />)

    await expect.element(screen.getByLabelText("Message")).toBeDisabled()
    await expect.element(screen.getByRole("button", { name: "Send" })).toBeDisabled()
  })
})

describe("Sender slash-command menu", () => {
  it("opens the command menu when the user types a leading slash", async () => {
    const screen = await render(<Sender onSubmit={vi.fn()} commands={COMMANDS} interactionMode="default" onInteractionModeChange={vi.fn()} />)

    await userEvent.type(screen.getByLabelText("Message"), "/")

    await expect.element(screen.getByText("/compact")).toBeInTheDocument()
    await expect.element(screen.getByText("/fork")).toBeInTheDocument()
  })

  it("opens the same menu from the toolbar button, including Model/Permission", async () => {
    const screen = await render(<Sender onSubmit={vi.fn()} commands={COMMANDS} interactionMode="default" onInteractionModeChange={vi.fn()} />)

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))

    await expect.element(screen.getByText("Model")).toBeInTheDocument()
    await expect.element(screen.getByText("/compact")).toBeInTheDocument()
  })

  it("inserts a control command into the input when selected", async () => {
    const screen = await render(<Sender onSubmit={vi.fn()} commands={COMMANDS} interactionMode="default" onInteractionModeChange={vi.fn()} />)

    await userEvent.type(screen.getByLabelText("Message"), "/")
    await userEvent.click(screen.getByText("/compact"))

    await expect.element(screen.getByLabelText("Message")).toHaveValue("/compact")
  })

  it("prefixes a prompt skill command when selected from the toolbar", async () => {
    // Opening from the toolbar leaves the textarea empty, so a Prompt skill
    // command (e.g. /summarize) seeds `/summarize ` for further typing. (`/plan`
    // is special — it toggles plan mode instead of seeding; see below.)
    const screen = await render(<Sender onSubmit={vi.fn()} commands={COMMANDS} interactionMode="default" onInteractionModeChange={vi.fn()} />)

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))
    await userEvent.click(screen.getByText("/summarize"))

    await expect.element(screen.getByLabelText("Message")).toHaveValue("/summarize ")
  })
})

describe("Sender plan-mode toggle", () => {
  it("toggles plan mode on when the `/plan` command is selected", async () => {
    const onSubmit = vi.fn()
    const onInteractionModeChange = vi.fn()

    const screen = await render(
      <Sender
        onSubmit={onSubmit}
        commands={COMMANDS}
        interactionMode="default"
        onInteractionModeChange={onInteractionModeChange}
      />,
    )

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))
    await userEvent.click(screen.getByText("/plan"))

    // Toggles default → plan and never seeds the composer or reaches the model.
    expect(onInteractionModeChange).toHaveBeenCalledWith("plan")
    expect(onSubmit).not.toHaveBeenCalled()
    await expect.element(screen.getByLabelText("Message")).toHaveValue("")
  })

  it("toggles plan back to default when already in plan mode", async () => {
    const onInteractionModeChange = vi.fn()

    const screen = await render(
      <Sender
        onSubmit={vi.fn()}
        commands={COMMANDS}
        interactionMode="plan"
        onInteractionModeChange={onInteractionModeChange}
      />,
    )

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))
    await userEvent.click(screen.getByText("/plan"))

    expect(onInteractionModeChange).toHaveBeenCalledWith("default")
  })

  it("exposes the interaction-mode selector in the Commands menu", async () => {
    const onInteractionModeChange = vi.fn()

    const screen = await render(
      <Sender
        onSubmit={vi.fn()}
        commands={COMMANDS}
        interactionMode="default"
        onInteractionModeChange={onInteractionModeChange}
      />,
    )

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))
    await userEvent.click(screen.getByTestId("assistant-interaction-mode-plan"))

    expect(onInteractionModeChange).toHaveBeenCalledWith("plan")
  })
})
