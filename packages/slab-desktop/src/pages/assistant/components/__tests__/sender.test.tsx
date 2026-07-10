import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import Sender from "../sender"

describe("Sender", () => {
  it("submits trimmed text and clears the textarea", async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn()

    render(<Sender onSubmit={onSubmit} />)

    const textarea = screen.getByLabelText("Message")
    await user.type(textarea, "  hello slab  ")
    await user.click(screen.getByRole("button", { name: "Send" }))

    expect(onSubmit).toHaveBeenCalledWith(
      "hello slab",
      expect.objectContaining({ files: [], effort: "off" }),
      expect.anything(),
    )
    expect(textarea).toHaveValue("")
  })

  it("does not submit empty text", async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn()

    render(<Sender onSubmit={onSubmit} />)

    await user.type(screen.getByLabelText("Message"), "   ")
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it("disables input while loading", () => {
    render(<Sender loading onSubmit={vi.fn()} />)

    expect(screen.getByLabelText("Message")).toBeDisabled()
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled()
  })
})
