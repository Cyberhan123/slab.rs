import { describe, expect, it } from "vitest"
import { render } from "vitest-browser-react"

import { classifyDiffLine, PatchDiffView } from "../patch-diff-view"

describe("classifyDiffLine", () => {
  it("classifies the apply-patch dialect", () => {
    expect(classifyDiffLine("*** Begin Patch")).toBe("meta")
    expect(classifyDiffLine("*** Update File: src/main.rs")).toBe("meta")
    expect(classifyDiffLine("*** End of File")).toBe("meta")
    expect(classifyDiffLine("@@ fn main")).toBe("hunk")
    expect(classifyDiffLine("@@")).toBe("hunk")
    expect(classifyDiffLine("+added line")).toBe("add")
    expect(classifyDiffLine("-removed line")).toBe("del")
    expect(classifyDiffLine(" context line")).toBe("context")
    expect(classifyDiffLine("plain prose")).toBe("plain")
  })

  it("treats unified-diff file headers as meta, not add/del", () => {
    expect(classifyDiffLine("+++ b/x.rs")).toBe("meta")
    expect(classifyDiffLine("--- a/x.rs")).toBe("meta")
  })

  it("treats heredoc wrapper lines as meta", () => {
    expect(classifyDiffLine("apply_patch <<'EOF'")).toBe("meta")
    expect(classifyDiffLine("<<'EOF'")).toBe("meta")
    expect(classifyDiffLine("<<EOF")).toBe("meta")
    expect(classifyDiffLine("EOF")).toBe("meta")
    expect(classifyDiffLine("PATCH")).toBe("meta")
  })
})

describe("PatchDiffView", () => {
  it("renders every line inside a pre with per-line coloring classes", async () => {
    const diff = [
      "*** Begin Patch",
      "*** Update File: a.txt",
      "@@ fn main",
      "-old",
      "+new",
      " trailing context",
    ].join("\n")
    const screen = await render(<PatchDiffView diff={diff} />)
    const pre = screen.getByTestId("patch-diff-view")
    expect(pre.element().tagName).toBe("PRE")
    expect(pre.element().textContent).toContain("*** Update File: a.txt")

    const spans = Array.from(pre.element().querySelectorAll("span"))
    expect(spans).toHaveLength(6)
    expect(spans[0].className).toContain("block")
    // `+new` gets the green add class, `-old` the red del class.
    expect(spans[3].className).toContain("text-red")
    expect(spans[4].className).toContain("text-green")
  })

  it("renders an empty diff without crashing", async () => {
    const screen = await render(<PatchDiffView diff="" />)
    expect(screen.getByTestId("patch-diff-view").element().textContent).not.toBeNull()
  })
})
