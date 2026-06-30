import { describe, expect, it } from "vitest"
import { cleanup, render, screen } from "@testing-library/react"

import { AgentProgress } from "./agent-progress"

describe("AgentProgress", () => {
  it("renders X/N and a native progress element when a plan exists", () => {
    render(<AgentProgress progress={{ total: 4, completed: 1 }} labels={{ progress: "Plan" }} />)

    expect(screen.getByText("1/4")).toBeDefined()
    const bar = screen.getByTestId("agent-progress-bar")
    expect(bar.tagName).toBe("PROGRESS")
    expect(bar.getAttribute("aria-label")).toBe("Plan")
  })

  it("clamps completion above the total", () => {
    render(<AgentProgress progress={{ total: 3, completed: 9 }} labels={{ progress: "Plan" }} />)

    expect(screen.getByText("3/3")).toBeDefined()
  })

  it("renders nothing when there is no plan yet", () => {
    const { container } = render(<AgentProgress progress={null} labels={{ progress: "Plan" }} />)

    expect(container.firstChild).toBeNull()
    cleanup()
  })
})
