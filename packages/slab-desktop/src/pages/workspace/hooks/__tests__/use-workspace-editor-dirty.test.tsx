import { act, renderHook } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import type { WorkspaceFileContent } from "@/lib/workspace-bridge"

const makeFile = (content: string): WorkspaceFileContent =>
  ({ content } as unknown as WorkspaceFileContent)

type DirtyProps = {
  workspaceRoot: string | null
  selectedFile: WorkspaceFileContent | null
  editorContent: string
}

const emitRef: { current: ((dirty: boolean) => void) | null } = { current: null }

vi.mock("../../lib/workspace-lsp", () => ({
  watchWorkspaceVscodeEditorDirty: (
    _root: string,
    onChange: (dirty: boolean) => void,
  ) => {
    emitRef.current = onChange
    // Emit an initial clean state like the real watcher does.
    onChange(false)
    return Promise.resolve({ dispose: () => {} })
  },
}))

// Imported after the mock is registered so the hook picks up the mocked watcher.
import { useWorkspaceEditorDirty } from "../use-workspace-editor-dirty"

describe("useWorkspaceEditorDirty", () => {
  it("falls back to React state comparison when Monaco is unavailable (browser mode)", () => {
    // workspaceRoot === null -> the VS Code watcher never starts.
    const initial: { props: DirtyProps } = {
      props: {
        workspaceRoot: null,
        selectedFile: makeFile("original"),
        editorContent: "original",
      },
    }
    const { result, rerender } = renderHook(
      ({ props }: { props: DirtyProps }) => useWorkspaceEditorDirty(props),
      { initialProps: initial },
    )

    expect(result.current).toBe(false)

    // React editorContent diverges from disk -> dirty.
    rerender({
      props: {
        workspaceRoot: null,
        selectedFile: makeFile("original"),
        editorContent: "edited",
      },
    })
    expect(result.current).toBe(true)

    // No active file -> never dirty.
    rerender({
      props: {
        workspaceRoot: null,
        selectedFile: null,
        editorContent: "edited",
      },
    })
    expect(result.current).toBe(false)
  })

  it("treats a dirty Monaco working copy as dirty even when React state matches disk", async () => {
    const initial: { props: DirtyProps } = {
      props: {
        workspaceRoot: "/workspace",
        // React thinks the buffer matches disk...
        selectedFile: makeFile("same"),
        editorContent: "same",
      },
    }
    const { result } = renderHook(
      ({ props }: { props: DirtyProps }) => useWorkspaceEditorDirty(props),
      { initialProps: initial },
    )

    // Allow the async watcher promise to resolve before emitting.
    await act(async () => {
      await Promise.resolve()
    })

    // ...but the embedded VS Code editor reports unsaved edits.
    await act(async () => {
      emitRef.current?.(true)
    })
    expect(result.current).toBe(true)

    // Clearing Monaco dirty falls back to the (clean) React comparison.
    await act(async () => {
      emitRef.current?.(false)
    })
    expect(result.current).toBe(false)
  })
})
