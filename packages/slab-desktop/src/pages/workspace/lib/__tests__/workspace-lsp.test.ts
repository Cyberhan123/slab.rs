import { describe, expect, it } from "vitest"

import type { WorkspaceFileEntry } from "@/lib/workspace-bridge"
import {
  workspaceVscodeDirtyCloseTarget,
  supportsWorkspaceLsp,
  workspaceLspDefinitionTargetFromResult,
  workspaceLspFileUri,
  workspaceLspImportSpecifierPositionForTarget,
  workspaceLspModelPath,
  workspaceLspRelativePathFromUri,
} from "../workspace-uri"
import {
  directoryAncestors,
  entryToTreeNode,
  insertChildren,
  languageForFile,
  lspLanguageForFile,
  sortDirectoryPaths,
  upsertFileTab,
} from "../workspace-page-utils"

describe("workspace LSP helpers", () => {
  it("matches supported language providers", () => {
    expect(supportsWorkspaceLsp("typescript")).toBe(true)
    expect(supportsWorkspaceLsp("typescriptreact")).toBe(true)
    expect(supportsWorkspaceLsp("javascriptreact")).toBe(true)
    expect(supportsWorkspaceLsp("python")).toBe(true)
    expect(supportsWorkspaceLsp("go")).toBe(true)
    expect(supportsWorkspaceLsp("rust")).toBe(true)
    expect(supportsWorkspaceLsp("markdown")).toBe(false)
    expect(supportsWorkspaceLsp("plaintext")).toBe(false)
  })

  it("keeps editor language compatible while using React LSP ids", () => {
    expect(languageForFile("component.tsx")).toBe("typescript")
    expect(languageForFile("component.jsx")).toBe("javascript")
    expect(lspLanguageForFile("component.tsx")).toBe("typescriptreact")
    expect(lspLanguageForFile("component.jsx")).toBe("javascriptreact")
  })

  it("routes native language files to plugin-capable LSP ids", () => {
    expect(languageForFile("src/main.rs")).toBe("rust")
    expect(languageForFile("cmd/server/main.go")).toBe("go")
    expect(languageForFile("scripts/tool.py")).toBe("python")
    expect(lspLanguageForFile("src/main.rs")).toBe("rust")
    expect(lspLanguageForFile("cmd/server/main.go")).toBe("go")
    expect(lspLanguageForFile("scripts/tool.py")).toBe("python")
  })

  it.each([
    ["Dockerfile", "dockerfile"],
    ["dockerfile", "dockerfile"],
    ["Makefile", "makefile"],
    ["GNUmakefile", "makefile"],
    [".env.local", "dotenv"],
    ["src/app.ts", "typescript"],
    ["src/app.mjs", "javascript"],
    ["src/app.cjs", "javascript"],
    ["src/App.java", "java"],
    ["include/slab.h", "c"],
    ["native/slab.cc", "cpp"],
    ["native/slab.cpp", "cpp"],
    ["native/slab.cxx", "cpp"],
    ["native/slab.hpp", "cpp"],
    ["settings.jsonc", "json"],
    ["docs/readme.mdx", "markdown"],
    ["styles/app.css", "css"],
    ["styles/app.scss", "scss"],
    ["styles/app.less", "less"],
    ["index.html", "html"],
    ["Cargo.toml", "toml"],
    ["scripts/install.sh", "shell"],
    ["scripts/install.bash", "shell"],
    ["scripts/install.zsh", "shell"],
    ["scripts/install.ps1", "powershell"],
    ["migrations/init.sql", "sql"],
    ["assets/icon.svg", "xml"],
    ["workflow.yaml", "yaml"],
    ["schema.graphql", "graphql"],
    ["schema.gql", "graphql"],
    ["Component.vue", "vue"],
    ["Component.svelte", "svelte"],
    ["tool.rb", "ruby"],
    ["index.php", "php"],
    ["script.lua", "lua"],
    ["analysis.r", "r"],
    ["App.swift", "swift"],
    ["Main.kt", "kotlin"],
    ["build.gradle.kts", "kotlin"],
    ["Program.cs", "csharp"],
    ["runtime.env", "dotenv"],
    ["settings.ini", "ini"],
    ["settings.cfg", "ini"],
    ["server.conf", "ini"],
    ["types.proto", "proto"],
    ["README", "plaintext"],
  ])("detects %s as %s", (fileName, language) => {
    expect(languageForFile(fileName)).toBe(language)
  })

  it("sorts directory paths by depth after removing duplicates and blanks", () => {
    expect(sortDirectoryPaths(["src/components", "src", "", "src/components", "docs/api", "docs"])).toEqual([
      "docs",
      "src",
      "docs/api",
      "src/components",
    ])
  })

  it("maps workspace entries into expandable tree nodes", () => {
    expect(entryToTreeNode(workspaceEntry({ kind: "directory", name: "src", relativePath: "src" }))).toEqual({
      id: "src",
      hasChildren: true,
      children: [],
      kind: "directory",
      loaded: false,
      name: "src",
      relativePath: "src",
    })
    expect(entryToTreeNode(workspaceEntry({ kind: "file", name: "main.rs", relativePath: "src/main.rs" }))).toEqual({
      id: "src/main.rs",
      hasChildren: false,
      children: undefined,
      kind: "file",
      loaded: true,
      name: "main.rs",
      relativePath: "src/main.rs",
    })
  })

  it("inserts children at nested tree paths without mutating siblings", () => {
    const nodes = [
      entryToTreeNode(workspaceEntry({ kind: "directory", name: "src", relativePath: "src" })),
      entryToTreeNode(workspaceEntry({ kind: "file", name: "README.md", relativePath: "README.md" })),
    ]
    const next = insertChildren(nodes, "src", [
      entryToTreeNode(workspaceEntry({ kind: "file", name: "main.rs", relativePath: "src/main.rs" })),
    ])

    expect(next[0]?.loaded).toBe(true)
    expect(next[0]?.children).toHaveLength(1)
    expect(next[1]).toBe(nodes[1])
    expect(nodes[0]?.children).toEqual([])
  })

  it("upserts file tabs and derives directory ancestors", () => {
    const tabs = [{ name: "a.ts", relativePath: "src/a.ts" }]
    expect(upsertFileTab(tabs, { name: "b.ts", relativePath: "src/b.ts" })).toHaveLength(2)
    expect(
      upsertFileTab(tabs, { name: "a.tsx", relativePath: "src/a.ts" }),
    ).toEqual([{ name: "a.tsx", relativePath: "src/a.ts" }])
    expect(directoryAncestors("src/pages/index.tsx")).toEqual(["src", "src/pages"])
    expect(directoryAncestors("src/pages", true)).toEqual(["src", "src/pages"])
    expect(directoryAncestors("")).toEqual([])
  })

  it("builds file uri model paths for Monaco", () => {
    expect(workspaceLspModelPath("C:\\Users\\demo\\repo", "src/index.ts")).toBe(
      "file:///c:/Users/demo/repo/src/index.ts",
    )
  })

  it("builds file uri model paths from Windows verbatim workspace roots", () => {
    expect(workspaceLspModelPath("\\\\?\\C:\\Users\\demo\\repo", "src/index.ts")).toBe(
      "file:///c:/Users/demo/repo/src/index.ts",
    )
  })

  it("builds normalized file uri roots for VS Code workspace folders", () => {
    expect(workspaceLspFileUri("C:\\Users\\demo\\repo")).toBe(
      "file:///c:/Users/demo/repo",
    )
    expect(workspaceLspFileUri("\\\\?\\C:\\Users\\demo\\repo")).toBe(
      "file:///c:/Users/demo/repo",
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

  it("maps workspace file uris under Windows verbatim workspace roots", () => {
    expect(
      workspaceLspRelativePathFromUri(
        "\\\\?\\C:\\Users\\demo\\repo",
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

  it("returns a dirty VS Code close target only for workspace files", () => {
    const input = { resource: { toString: () => "file:///C:/Users/demo/repo/src/index.ts" } }
    expect(
      workspaceVscodeDirtyCloseTarget(
        "C:\\Users\\demo\\repo",
        input,
        () => true,
      ),
    ).toBe("src/index.ts")
    expect(
      workspaceVscodeDirtyCloseTarget(
        "C:\\Users\\demo\\repo",
        input,
        () => false,
      ),
    ).toBeNull()
    expect(
      workspaceVscodeDirtyCloseTarget(
        "C:\\Users\\demo\\repo",
        { resource: { toString: () => "file:///C:/Users/demo/other/src/index.ts" } },
        () => true,
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

function workspaceEntry(
  entry: Pick<WorkspaceFileEntry, "kind" | "name" | "relativePath">,
): WorkspaceFileEntry {
  return {
    ...entry,
    id: entry.relativePath,
    hasChildren: entry.kind === "directory",
  }
}
