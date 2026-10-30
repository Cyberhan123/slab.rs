import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { BrainIcon, GlobeIcon, SquareTerminalIcon } from "lucide-react"

import {
  Trace,
  TraceCollapsible,
  TraceCollapsibleContent,
  TraceCollapsibleTrigger,
  TraceDescription,
  TraceIcon,
  TraceLine,
  TraceTitle,
} from "@/trace"
import { CodeBlock, CodeBlockContent } from "@/code-block"
import { renderComponentScene } from "../test-utils"

function TraceGallery() {
  return (
    <div
      data-testid="trace-gallery"
      className="flex w-96 flex-col gap-6 rounded-[28px] border border-border/60 bg-card p-6 shadow-sm"
    >
      <Trace data-testid="trace-run">
        <TraceLine status="running" data-tool-state="input-available">
          <TraceIcon>
            <SquareTerminalIcon />
          </TraceIcon>
          <TraceTitle>Bash: bun run test</TraceTitle>
        </TraceLine>
        <TraceLine status="error" data-tool-state="output-error">
          <TraceIcon>
            <GlobeIcon />
          </TraceIcon>
          <TraceTitle>Fetch: https://example.com</TraceTitle>
          <TraceDescription>Failed to run</TraceDescription>
        </TraceLine>
        <TraceLine data-tool-state="output-available">
          <TraceIcon>
            <GlobeIcon />
          </TraceIcon>
          <TraceTitle>Search: slab docs</TraceTitle>
          <TraceDescription>Ran</TraceDescription>
        </TraceLine>
      </Trace>
      <TraceCollapsible defaultOpen={false} data-testid="trace-collapsible">
        <TraceCollapsibleTrigger data-testid="trace-collapsible-trigger">
          <TraceLine status="running" data-tool-state="input-streaming">
            <TraceIcon>
              <BrainIcon />
            </TraceIcon>
            <TraceTitle>Thinking</TraceTitle>
          </TraceLine>
        </TraceCollapsibleTrigger>
        <TraceCollapsibleContent>
          <CodeBlock size="sm" data-testid="trace-collapsible-output">
            <CodeBlockContent wrap className="max-h-64">
              {"trace output line"}
            </CodeBlockContent>
          </CodeBlock>
        </TraceCollapsibleContent>
      </TraceCollapsible>
    </div>
  )
}

describe("Trace browser coverage", () => {
  it("matches the shared trace gallery screenshot", async () => {
    await renderComponentScene(<TraceGallery />)
    const trace = page.getByTestId("trace-run")
    await expect.element(trace).toBeVisible()
    await expect(page.getByTestId("trace-gallery")).toMatchScreenshot("trace-gallery.png")
  })

  it("toggles the collapsible via its trigger", async () => {
    await renderComponentScene(<TraceGallery />)
    const trigger = page.getByTestId("trace-collapsible-trigger")
    await expect.element(trigger).toBeVisible()
    await expect.element(page.getByTestId("trace-collapsible-output")).not.toBeInTheDocument()

    await trigger.click()
    await expect.element(page.getByTestId("trace-collapsible-output")).toBeVisible()
  })
})
