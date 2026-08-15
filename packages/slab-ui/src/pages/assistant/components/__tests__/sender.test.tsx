import { userEvent } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import { SlabProvider } from "../../../../provider/slab-provider"
import { createTestSlabPorts } from "../../../../provider/test-ports"

import Sender from "../sender"

import type { ReactElement } from "react"
function renderSender(ui: ReactElement) {
  return render(
    <SlabProvider deps={{ ports: createTestSlabPorts() }}>{ui}</SlabProvider>,
  )
}
import type { CommandInfo } from "@slab/core/harness"

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

    const screen = await renderSender(
      <Sender onSubmit={onSubmit} commands={COMMANDS} planMode={false} onPlanModeChange={vi.fn()} />,
    )

    const textarea = screen.getByLabelText("Message")
    await userEvent.type(textarea, "  hello slab  ")
    await userEvent.click(screen.getByRole("button", { name: "Send" }))

    expect(onSubmit).toHaveBeenCalledWith(
      "hello slab",
      expect.objectContaining({ files: [], effort: "off", agentType: undefined }),
      expect.anything(),
    )
    await expect.element(textarea).toHaveValue("")
  })

  it("does not submit empty text", async () => {
    const onSubmit = vi.fn()

    const screen = await renderSender(
      <Sender onSubmit={onSubmit} commands={COMMANDS} planMode={false} onPlanModeChange={vi.fn()} />,
    )

    await userEvent.type(screen.getByLabelText("Message"), "   ")
    await expect.element(screen.getByRole("button", { name: "Send" })).toBeDisabled()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it("disables input while loading", async () => {
    const screen = await renderSender(
      <Sender loading onSubmit={vi.fn()} commands={COMMANDS} planMode={false} onPlanModeChange={vi.fn()} />,
    )

    await expect.element(screen.getByLabelText("Message")).toBeDisabled()
    await expect.element(screen.getByRole("button", { name: "Send" })).toBeDisabled()
  })
})

describe("Sender slash-command menu", () => {
  it("opens the command menu when the user types a leading slash", async () => {
    const screen = await renderSender(
      <Sender onSubmit={vi.fn()} commands={COMMANDS} planMode={false} onPlanModeChange={vi.fn()} />,
    )

    await userEvent.type(screen.getByLabelText("Message"), "/")

    await expect.element(screen.getByText("/compact")).toBeInTheDocument()
    await expect.element(screen.getByText("/fork")).toBeInTheDocument()
  })

  it("opens the same menu from the toolbar button, including the Model group", async () => {
    const screen = await renderSender(
      <Sender onSubmit={vi.fn()} commands={COMMANDS} planMode={false} onPlanModeChange={vi.fn()} />,
    )

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))

    await expect.element(screen.getByText("Model")).toBeInTheDocument()
    await expect.element(screen.getByText("/compact")).toBeInTheDocument()
  })

  it("inserts a control command into the input when selected", async () => {
    const screen = await renderSender(
      <Sender onSubmit={vi.fn()} commands={COMMANDS} planMode={false} onPlanModeChange={vi.fn()} />,
    )

    await userEvent.type(screen.getByLabelText("Message"), "/")
    await userEvent.click(screen.getByText("/compact"))

    await expect.element(screen.getByLabelText("Message")).toHaveValue("/compact")
  })

  it("prefixes a prompt skill command when selected from the toolbar", async () => {
    // Opening from the toolbar leaves the textarea empty, so a Prompt skill
    // command (e.g. /summarize) seeds `/summarize ` for further typing. (`/plan`
    // is special — it toggles plan mode instead of seeding; see below.)
    const screen = await renderSender(
      <Sender onSubmit={vi.fn()} commands={COMMANDS} planMode={false} onPlanModeChange={vi.fn()} />,
    )

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))
    await userEvent.click(screen.getByText("/summarize"))

    await expect.element(screen.getByLabelText("Message")).toHaveValue("/summarize ")
  })
})

describe("Sender plan-mode toggle", () => {
  it("toggles plan mode on when the `/plan` command is selected", async () => {
    const onSubmit = vi.fn()
    const onPlanModeChange = vi.fn()

    const screen = await renderSender(
      <Sender onSubmit={onSubmit} commands={COMMANDS} planMode={false} onPlanModeChange={onPlanModeChange} />,
    )

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))
    await userEvent.click(screen.getByText("/plan"))

    // Toggles plan off → on and never seeds the composer or reaches the model.
    expect(onPlanModeChange).toHaveBeenCalledWith(true)
    expect(onSubmit).not.toHaveBeenCalled()
    await expect.element(screen.getByLabelText("Message")).toHaveValue("")
  })

  it("toggles plan mode off when already on", async () => {
    const onPlanModeChange = vi.fn()

    const screen = await renderSender(
      <Sender onSubmit={vi.fn()} commands={COMMANDS} planMode={true} onPlanModeChange={onPlanModeChange} />,
    )

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))
    await userEvent.click(screen.getByText("/plan"))

    expect(onPlanModeChange).toHaveBeenCalledWith(false)
  })

  it("renders the plan chip when plan mode is on and clears it via the X", async () => {
    const onPlanModeChange = vi.fn()

    const screen = await renderSender(
      <Sender onSubmit={vi.fn()} commands={COMMANDS} planMode={true} onPlanModeChange={onPlanModeChange} />,
    )

    const chip = screen.getByTestId("assistant-plan-mode-chip")
    await expect.element(chip).toBeInTheDocument()

    await userEvent.click(chip)

    expect(onPlanModeChange).toHaveBeenCalledWith(false)
  })

  it("sends agentType 'plan' on submit when plan mode is on", async () => {
    const onSubmit = vi.fn()

    const screen = await renderSender(
      <Sender onSubmit={onSubmit} commands={COMMANDS} planMode={true} onPlanModeChange={vi.fn()} />,
    )

    await userEvent.type(screen.getByLabelText("Message"), "plan this")
    await userEvent.click(screen.getByRole("button", { name: "Send" }))

    expect(onSubmit).toHaveBeenCalledWith(
      "plan this",
      expect.objectContaining({ agentType: "plan" }),
      expect.anything(),
    )
  })
})

describe("Sender permission-mode selector", () => {
  it("exposes a dedicated permission button that updates the selected mode", async () => {
    const onSubmit = vi.fn()

    const screen = await renderSender(
      <Sender onSubmit={onSubmit} commands={COMMANDS} planMode={false} onPlanModeChange={vi.fn()} />,
    )

    // Open the dedicated permission popover (left of Send).
    await userEvent.click(screen.getByTestId("assistant-permission-mode-trigger"))
    await userEvent.click(screen.getByTestId("assistant-permission-mode-full_control"))

    await userEvent.type(screen.getByLabelText("Message"), "do work")
    await userEvent.click(screen.getByRole("button", { name: "Send" }))

    expect(onSubmit).toHaveBeenCalledWith(
      "do work",
      expect.objectContaining({ permissionMode: "full_control" }),
      expect.anything(),
    )
  })
})
