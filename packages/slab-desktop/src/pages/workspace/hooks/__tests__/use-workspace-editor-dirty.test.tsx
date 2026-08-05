import { renderHook } from "vitest-browser-react"
import { beforeAll, describe, expect, it, vi } from "vitest"

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
  // The hook installs its watcher via a fire-and-forget
  // `import("../lib/workspace-editor")`. In browser mode that dynamic import is
  // served by the Vite dev server on a macrotask, so without warming it only
  // resolves during teardown and the watcher never attaches in time. Awaiting the
  // mocked module here caches it so the hook's import resolves on a microtask.
  beforeAll(async () => {
    await import("../../lib/workspace-editor")
  })

  it("is never dirty when no workspace is open (no editor, no watcher)", async () => {
    // workspaceRoot === null -> the VS Code watcher never starts and there is no
    // editor surface, so the file can never be dirty regardless of selection.
    const initial: { props: DirtyProps } = {
      props: {
        workspaceRoot: null,
        selectedFile: makeFile("original"),
      },
    }
    const { result, rerender } = await renderHook(
      (initialProps?: { props: DirtyProps }) => useWorkspaceEditorDirty(initialProps!.props),
      { initialProps: initial },
    )

    expect(result.current).toBe(false)

    // rerender() already wraps its render in act() internally, so wrapping it
    // again here would re-enter act and corrupt the act environment for later
    // tests (their useEffect would stop flushing). Await it directly instead.
    await rerender({
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
    const { result, act } = await renderHook(
      (initialProps?: { props: DirtyProps }) => useWorkspaceEditorDirty(initialProps!.props),
      { initialProps: initial },
    )

    // Allow the async watcher promise to resolve before emitting. The watcher is
    // installed via a fire-and-forget dynamic import; the beforeAll cache-warm
    // above makes it resolve on a microtask so a single flush is enough.
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
    const { result, rerender, act } = await renderHook(
      (initialProps?: { props: DirtyProps }) => useWorkspaceEditorDirty(initialProps!.props),
      { initialProps: initial },
    )

    // Allow the async watcher promise to resolve before emitting (see note above).
    await act(async () => {
      await Promise.resolve()
    })
    await act(async () => {
      emitRef.current?.(true)
    })
    expect(result.current).toBe(true)

    // No active file -> the stale dirty signal must reset so it cannot leak into
    // the next file's discard guard. Await rerender directly (see note above).
    await rerender({
      props: {
        workspaceRoot: "/workspace",
        selectedFile: null,
      },
    })
    expect(result.current).toBe(false)
  })
})
