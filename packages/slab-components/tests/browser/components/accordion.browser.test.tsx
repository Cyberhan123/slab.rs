import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"
import { Accordion, AccordionItem, AccordionTrigger, AccordionContent } from "@/accordion"
import { renderComponentScene } from "../test-utils"

function AccordionGallery() {
  return (
    <div data-testid="accordion-gallery" className="w-full max-w-md rounded-[28px] border border-border/60 bg-surface-1 p-6 shadow-sm">
      <Accordion type="single" collapsible defaultValue="item-1" data-testid="accordion">
        <AccordionItem value="item-1">
          <AccordionTrigger>Is it accessible?</AccordionTrigger>
          <AccordionContent>
            Yes. It adheres to the WAI-ARIA design pattern.
          </AccordionContent>
        </AccordionItem>
        <AccordionItem value="item-2">
          <AccordionTrigger>Is it styled?</AccordionTrigger>
          <AccordionContent>
            Yes. It comes with default styles that match the other components.
          </AccordionContent>
        </AccordionItem>
        <AccordionItem value="item-3">
          <AccordionTrigger>Is it animated?</AccordionTrigger>
          <AccordionContent>
            Yes. It's animated by default using CSS transitions.
          </AccordionContent>
        </AccordionItem>
      </Accordion>
    </div>
  )
}

describe("Accordion browser coverage", () => {
  it("matches the shared accordion gallery screenshot", async () => {
    await renderComponentScene(<AccordionGallery />)
    const accordion = page.getByTestId("accordion")
    await expect.element(accordion).toBeVisible()
    // Move the pointer onto the neutral scene container so the first trigger
    // doesn't inherit a stray hover underline from prior browser tests.
    await page.getByTestId("component-browser-scene").hover()
    await expect(page.getByTestId("accordion-gallery")).toMatchScreenshot("accordion-gallery.png")
  })

  it("keeps interactive accordion expand/collapse behavior covered", async () => {
    await renderComponentScene(
      <Accordion type="single" collapsible>
        <AccordionItem value="item-1">
          <AccordionTrigger data-testid="accordion-trigger">First item</AccordionTrigger>
          <AccordionContent data-testid="accordion-content">First content</AccordionContent>
        </AccordionItem>
        <AccordionItem value="item-2">
          <AccordionTrigger>Second item</AccordionTrigger>
          <AccordionContent>Second content</AccordionContent>
        </AccordionItem>
      </Accordion>
    )
    const trigger = page.getByTestId("accordion-trigger").first()
    await expect.element(trigger).toBeVisible()
    await trigger.click()
    // Verify the content became visible after clicking
    const content = page.getByTestId("accordion-content").first()
    await expect.element(content).toBeVisible()
  })
})
