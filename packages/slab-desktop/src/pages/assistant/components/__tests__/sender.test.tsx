import { userEvent } from "vitest/browser"
import { describe, expect, it, vi } from "vitest"
import { render } from "vitest-browser-react"

import Sender from "../sender"

describe("Sender", () => {
  it("submits trimmed text and clears the textarea", async () => {
    const onSubmit = vi.fn()

    const screen = await render(<Sender onSubmit={onSubmit} />)

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

    const screen = await render(<Sender onSubmit={onSubmit} />)

    await userEvent.type(screen.getByLabelText("Message"), "   ")
    await expect.element(screen.getByRole("button", { name: "Send" })).toBeDisabled()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it("disables input while loading", async () => {
    const screen = await render(<Sender loading onSubmit={vi.fn()} />)

    await expect.element(screen.getByLabelText("Message")).toBeDisabled()
    await expect.element(screen.getByRole("button", { name: "Send" })).toBeDisabled()
  })
})

describe("Sender slash-command menu", () => {
  it("opens the command menu when the user types a leading slash", async () => {
    const screen = await render(<Sender onSubmit={vi.fn()} />)

    await userEvent.type(screen.getByLabelText("Message"), "/")

    await expect.element(screen.getByText("/compact")).toBeInTheDocument()
    await expect.element(screen.getByText("/fork")).toBeInTheDocument()
  })

  it("opens the same menu from the toolbar button, including Model/Permission", async () => {
    const screen = await render(<Sender onSubmit={vi.fn()} />)

    await userEvent.click(screen.getByRole("button", { name: "Commands" }))

    await expect.element(screen.getByText("Model")).toBeInTheDocument()
    await expect.element(screen.getByText("/compact")).toBeInTheDocument()
  })

  it("inserts a command into the input when selected", async () => {
    const screen = await render(<Sender onSubmit={vi.fn()} />)

    await userEvent.type(screen.getByLabelText("Message"), "/")
    await userEvent.click(screen.getByText("/compact"))

    await expect.element(screen.getByLabelText("Message")).toHaveValue("/compact")
  })
})
