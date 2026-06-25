import { act, renderHook } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import type { WorkspaceFileContent } from "@/lib/workspace-bridge"

const makeFile = (content: string): WorkspaceFileContent =>
  ({ content } as unknown as WorkspaceFileContent)

type DirtyProps = {
  workspaceRoot: string | null
  selectedFile: WorkspaceFileContent | null
}

const emitRef: { current: ((dirty: boolean) => void) | null } = { current: null }

vi.mock("../../lib/workspace-editor", () => ({
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
  it("is never dirty when no workspace is open (no editor, no watcher)", () => {
    // workspaceRoot === null -> the VS Code watcher never starts and there is no
    // editor surface, so the file can never be dirty regardless of selection.
    const initial: { props: DirtyProps } = {
      props: {
        workspaceRoot: null,
        selectedFile: makeFile("original"),
      },
    }
    const { result, rerender } = renderHook(
      ({ props }: { props: DirtyProps }) => useWorkspaceEditorDirty(props),
      { initialProps: initial },
    )

    expect(result.current).toBe(false)

    rerender({
      props: {
        workspaceRoot: null,
        selectedFile: null,
      },
    })
    expect(result.current).toBe(false)
  })

  it("treats a dirty Monaco working copy as dirty", async () => {
    const initial: { props: DirtyProps } = {
      props: {
        workspaceRoot: "/workspace",
        selectedFile: makeFile("same"),
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

    // The embedded VS Code editor reports unsaved edits.
    await act(async () => {
      emitRef.current?.(true)
    })
    expect(result.current).toBe(true)

    // Clearing Monaco dirty returns to clean.
    await act(async () => {
      emitRef.current?.(false)
    })
    expect(result.current).toBe(false)
  })

  it("clears a stale dirty signal when the active file is unset", async () => {
    const initial: { props: DirtyProps } = {
      props: {
        workspaceRoot: "/workspace",
        selectedFile: makeFile("same"),
      },
    }
    const { result, rerender } = renderHook(
      ({ props }: { props: DirtyProps }) => useWorkspaceEditorDirty(props),
      { initialProps: initial },
    )

    await act(async () => {
      await Promise.resolve()
    })
    await act(async () => {
      emitRef.current?.(true)
    })
    expect(result.current).toBe(true)

    // No active file -> the stale dirty signal must reset so it cannot leak into
    // the next file's discard guard.
    rerender({
      props: {
        workspaceRoot: "/workspace",
        selectedFile: null,
      },
    })
    expect(result.current).toBe(false)
  })
})
