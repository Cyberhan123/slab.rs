import { page } from "vitest/browser"
import { describe, expect, it } from "vitest"

import {
  CodeBlock,
  CodeBlockActions,
  CodeBlockContent,
  CodeBlockCopyButton,
  CodeBlockHeader,
  CodeBlockLine,
  CodeBlockTitle,
} from "@/code-block"
import { renderComponentScene } from "../test-utils"

const SAMPLE = `fn main() {\n    println!("Hello, slab!");\n}`

function CodeBlockGallery() {
  return (
    <div
      data-testid="code-block-gallery"
      className="flex w-[28rem] flex-col gap-6 rounded-[28px] border border-border/60 bg-card p-6 shadow-sm"
    >
      <CodeBlock data-testid="code-block-headered">
        <CodeBlockHeader>
          <CodeBlockTitle>main.rs</CodeBlockTitle>
          <CodeBlockActions>
            <CodeBlockCopyButton value={SAMPLE} />
          </CodeBlockActions>
        </CodeBlockHeader>
        <CodeBlockContent lineNumbers startLine={10}>
          {SAMPLE}
        </CodeBlockContent>
      </CodeBlock>
      <CodeBlock size="sm">
        <CodeBlockContent wrap className="max-h-72">
          {"long tool output that should wrap instead of scrolling horizontally"}
        </CodeBlockContent>
      </CodeBlock>
      <CodeBlock variant="destructive">
        <CodeBlockContent wrap>{"error: command failed with exit code 124"}</CodeBlockContent>
      </CodeBlock>
      <CodeBlock>
        <CodeBlockContent lineNumbers>
          <CodeBlockLine variant="removed">{"- const stale = true"}</CodeBlockLine>
          <CodeBlockLine variant="added">{"+ const fresh = false"}</CodeBlockLine>
          <CodeBlockLine variant="highlighted">{"  export { fresh }"}</CodeBlockLine>
        </CodeBlockContent>
      </CodeBlock>
    </div>
  )
}

describe("CodeBlock browser coverage", () => {
  it("matches the shared code-block gallery screenshot", async () => {
    await renderComponentScene(<CodeBlockGallery />)
    const headered = page.getByTestId("code-block-headered")
    await expect.element(headered).toBeVisible()
    await expect(page.getByTestId("code-block-gallery")).toMatchScreenshot("code-block-gallery.png")
  })
})
