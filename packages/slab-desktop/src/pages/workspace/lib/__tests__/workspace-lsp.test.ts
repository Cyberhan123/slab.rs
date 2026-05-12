import { describe, expect, it } from "vitest"

import {
  supportsWorkspaceLsp,
  workspaceLspDefinitionTargetFromResult,
  workspaceLspImportSpecifierPositionForTarget,
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

  it("maps definition locations to workspace files", () => {
    expect(
      workspaceLspDefinitionTargetFromResult("C:\\Users\\demo\\repo", {
        range: {
          end: { character: 12, line: 9 },
          start: { character: 4, line: 9 },
        },
        uri: { toString: () => "file:///C:/Users/demo/repo/src/target.ts" },
      }),
    ).toEqual({
      endColumn: 13,
      endLineNumber: 10,
      relativePath: "src/target.ts",
      startColumn: 5,
      startLineNumber: 10,
    })
  })

  it("maps definition location arrays to workspace files", () => {
    expect(
      workspaceLspDefinitionTargetFromResult("C:\\Users\\demo\\repo", [
        {
          range: {
            end: { character: 12, line: 9 },
            start: { character: 4, line: 9 },
          },
          uri: { toString: () => "file:///C:/Users/demo/repo/src/target.ts" },
        },
      ]),
    ).toEqual({
      endColumn: 13,
      endLineNumber: 10,
      relativePath: "src/target.ts",
      startColumn: 5,
      startLineNumber: 10,
    })
  })

  it("maps definition links to target selection ranges", () => {
    expect(
      workspaceLspDefinitionTargetFromResult("C:\\Users\\demo\\repo", [
        {
          targetSelectionRange: {
            end: { character: 8, line: 3 },
            start: { character: 2, line: 3 },
          },
          targetUri: "file:///C:/Users/demo/repo/src/link-target.ts",
        },
      ]),
    ).toEqual({
      endColumn: 9,
      endLineNumber: 4,
      relativePath: "src/link-target.ts",
      startColumn: 3,
      startLineNumber: 4,
    })
  })

  it("ignores definition targets outside the workspace", () => {
    expect(
      workspaceLspDefinitionTargetFromResult("C:\\Users\\demo\\repo", [
        {
          range: {
            end: { character: 1, line: 1 },
            start: { character: 1, line: 1 },
          },
          uri: "file:///C:/Users/demo/other/src/index.ts",
        },
      ]),
    ).toBeNull()
  })

  it("finds the import module specifier for alias definition targets", () => {
    expect(
      workspaceLspImportSpecifierPositionForTarget(
        "import { useAudio } from './hooks/use-audio';",
        {
          endColumn: 18,
          endLineNumber: 1,
          relativePath: "src/index.tsx",
          startColumn: 10,
          startLineNumber: 1,
        },
      ),
    ).toEqual({
      column: 27,
      lineNumber: 1,
    })
  })

  it("does not treat non-import definition targets as module specifiers", () => {
    expect(
      workspaceLspImportSpecifierPositionForTarget(
        "const state = useAudio();",
        {
          endColumn: 23,
          endLineNumber: 1,
          relativePath: "src/index.tsx",
          startColumn: 15,
          startLineNumber: 1,
        },
      ),
    ).toBeNull()
  })
})
