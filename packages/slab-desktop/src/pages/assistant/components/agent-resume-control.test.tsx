import { describe, expect, it, vi } from "vitest"
import { cleanup, fireEvent, render, screen } from "@testing-library/react"

import { AgentResumeControl } from "./agent-resume-control"

describe("AgentResumeControl", () => {
  it("renders the reason copy and a resume button that fires onResume", () => {
    const onResume = vi.fn<() => void>()
    render(
      <AgentResumeControl
        reason="max_turns_reached"
        onResume={onResume}
        labels={{ resume: "Resume" }}
      />
    )

    expect(screen.getByText("Reached the turn limit")).toBeDefined()
    fireEvent.click(screen.getByTestId("agent-resume-button"))
    expect(onResume).toHaveBeenCalledOnce()
  })

  it("renders nothing for a non-resumable reason", () => {
    const { container } = render(
      <AgentResumeControl reason="completed" onResume={() => {}} labels={{ resume: "Resume" }} />
    )

    expect(container.firstChild).toBeNull()
    cleanup()
  })

  it("renders nothing when no reason is provided", () => {
    const { container } = render(
      <AgentResumeControl reason={null} onResume={() => {}} labels={{ resume: "Resume" }} />
    )

    expect(container.firstChild).toBeNull()
    cleanup()
  })
})
