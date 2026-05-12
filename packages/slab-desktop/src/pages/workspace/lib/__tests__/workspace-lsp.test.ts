import { describe, expect, it } from "vitest"

import {
  supportsWorkspaceLsp,
  workspaceLspModelPath,
  workspaceLspRelativePathFromUri,
} from "../workspace-lsp-utils"
import { languageForFile, lspLanguageForFile } from "../workspace-page-utils"

describe("workspace LSP helpers", () => {
  it("matches supported language providers", () => {
    expect(supportsWorkspaceLsp("typescript")).toBe(true)
    expect(supportsWorkspaceLsp("typescriptreact")).toBe(true)
    expect(supportsWorkspaceLsp("javascriptreact")).toBe(true)
    expect(supportsWorkspaceLsp("python")).toBe(true)
    expect(supportsWorkspaceLsp("markdown")).toBe(false)
    expect(supportsWorkspaceLsp("plaintext")).toBe(false)
  })

  it("keeps editor language compatible while using React LSP ids", () => {
    expect(languageForFile("component.tsx")).toBe("typescript")
    expect(languageForFile("component.jsx")).toBe("javascript")
    expect(lspLanguageForFile("component.tsx")).toBe("typescriptreact")
    expect(lspLanguageForFile("component.jsx")).toBe("javascriptreact")
  })

  it("builds file uri model paths for Monaco", () => {
    expect(workspaceLspModelPath("C:\\Users\\demo\\repo", "src/index.ts")).toBe(
      "file:///c:/Users/demo/repo/src/index.ts",
    )
  })

  it("maps workspace file uris back to relative paths", () => {
    expect(
      workspaceLspRelativePathFromUri(
        "C:\\Users\\demo\\repo",
        "file:///C:/Users/demo/repo/src/index.ts",
      ),
    ).toBe("src/index.ts")
  })

  it("handles lowercase drive letters emitted by Monaco URIs", () => {
    expect(
      workspaceLspRelativePathFromUri(
        "C:\\Users\\demo\\repo",
        "file:///c:/Users/demo/repo/src/index.ts",
      ),
    ).toBe("src/index.ts")
  })

  it("handles encoded drive letters emitted by Monaco URIs", () => {
    expect(
      workspaceLspRelativePathFromUri(
        "C:\\Users\\demo\\repo",
        "file:///c%3A/Users/demo/repo/src/index.ts",
      ),
    ).toBe("src/index.ts")
  })

  it("decodes escaped file uri paths", () => {
    expect(
      workspaceLspRelativePathFromUri(
        "C:\\Users\\demo\\repo",
        "file:///C:/Users/demo/repo/src/my%20file.ts",
      ),
    ).toBe("src/my file.ts")
  })

  it("rejects file uris outside the workspace", () => {
    expect(
      workspaceLspRelativePathFromUri(
        "C:\\Users\\demo\\repo",
        "file:///C:/Users/demo/other/src/index.ts",
      ),
    ).toBeNull()
  })
})
