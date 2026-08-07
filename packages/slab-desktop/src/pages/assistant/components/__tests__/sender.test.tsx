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
]

describe("Sender", () => {
  it("submits trimmed text and clears the textarea", async () => {
    const onSubmit = vi.fn()

    const screen = await render(<Sender onSubmit={onSubmit} commands={COMMANDS} />)

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

    const screen = await render(<Sender onSubmit={onSubmit} commands={COMMANDS} />)

    await userEvent.type(screen.getByLabelText("Message"), "   ")
    await expect.element(screen.getByRole("button", { name: "Send" })).toBeDisabled()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it("disables input while loading", async () => {
    const screen = await render(<Sender loading onSubmit={vi.fn()} commands={COMMANDS} />)

    await expect.element(screen.getByLabelText("Message")).toBeDisabled()
    await expect.element(screen.getByRole("button", { name: "Send" })).toBeDisabled()
  })
})

describe("Sender slash-command menu", () => {
  it("opens the command menu when the user types a leading slash", async () => {
    const screen = await render(<Sender onSubmit={vi.fn()} commands={COMMANDS} />)

    await userEvent.type(screen.getByLabelText("Message"), "/")

    await expect.element(screen.getByText("/compact")).toBeInTheDocument()
    await expect.element(screen.getByText("/fork")).toBeInTheDocument()
  })

  it("opens the same menu from the toolbar button, including Model/Permission", async () => {
    const screen = await render(<Sender onSubmit={vi.fn()} commands={COMMANDS} />)

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))

    await expect.element(screen.getByText("Model")).toBeInTheDocument()
    await expect.element(screen.getByText("/compact")).toBeInTheDocument()
  })

  it("inserts a control command into the input when selected", async () => {
    const screen = await render(<Sender onSubmit={vi.fn()} commands={COMMANDS} />)

    await userEvent.type(screen.getByLabelText("Message"), "/")
    await userEvent.click(screen.getByText("/compact"))

    await expect.element(screen.getByLabelText("Message")).toHaveValue("/compact")
  })

  it("prefixes a prompt command when selected from the toolbar", async () => {
    // Opening from the toolbar leaves the textarea empty, so a Prompt command
    // (e.g. /plan) seeds `/plan ` for further typing.
    const screen = await render(<Sender onSubmit={vi.fn()} commands={COMMANDS} />)

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))
    await userEvent.click(screen.getByText("/plan"))

    await expect.element(screen.getByLabelText("Message")).toHaveValue("/plan ")
  })
})
