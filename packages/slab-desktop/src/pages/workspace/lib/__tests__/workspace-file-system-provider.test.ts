import { describe, expect, it, vi } from "vitest"

// The provider depends on the Monaco/VS Code service graph (URI, Emitter, the
// files-service-override enums and OverlayFileSystemProvider). Those packages
// import their own `.css`, which vitest cannot load as native ESM, so this repo
// keeps Monaco out of unit tests. We stub only the runtime primitives the
// provider logic touches; TypeScript still resolves the real types.
vi.mock("@codingame/monaco-vscode-api/vscode/vs/base/common/uri", () => ({
  URI: {
    parse: (value: string) => ({ toString: () => value }),
  },
}))
vi.mock("@codingame/monaco-vscode-api/vscode/vs/base/common/event", () => ({
  Emitter: class TestEmitter<T> {
    private listeners: Array<(event: T) => void> = []
    readonly event = (listener: (event: T) => void) => {
      this.listeners = [...this.listeners, listener]
      return {
        dispose: () => {
          this.listeners = this.listeners.filter((item) => item !== listener)
        },
      }
    }
    fire(event: T) {
      for (const listener of this.listeners) {
        listener(event)
      }
    }
  },
}))
vi.mock("@codingame/monaco-vscode-files-service-override", () => ({
  FileChangeType: { UPDATED: 0, ADDED: 1, DELETED: 2 },
  FileSystemProviderCapabilities: { FileReadWrite: 2 },
  FileSystemProviderError: { create: (message: string) => new Error(message) },
  FileSystemProviderErrorCode: {
    FileNotFound: "EntryNotFound",
    NoPermissions: "NoPermissions",
    Unavailable: "Unavailable",
  },
  FileType: { Unknown: 0, File: 1, Directory: 2, SymbolicLink: 64 },
  OverlayFileSystemProvider: class {
    register() {
      return { dispose() {} }
    }
  },
  registerFileSystemOverlay: () => ({ dispose() {} }),
}))

import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri"
import {
  FileChangeType,
  FileType,
} from "@codingame/monaco-vscode-files-service-override"
import {
  SlabRemoteFileSystemProvider,
  SlabWorkspaceBackendFileSystemProvider,
  type SlabWorkspaceBackendBridge,
} from "../workspace-file-system-provider"
import { workspaceLspModelPath } from "../workspace-uri"
import type {
  WorkspaceFileEntry,
  WorkspacePathMetadata,
} from "@/lib/workspace-bridge"

const WORKSPACE_ROOT = "C:\\test\\repo"

function resourceFor(relativePath: string): URI {
  return URI.parse(workspaceLspModelPath(WORKSPACE_ROOT, relativePath))
}

function fileEntry(
  entry: Pick<WorkspaceFileEntry, "kind" | "name" | "relativePath"> &
    Partial<WorkspaceFileEntry>,
): WorkspaceFileEntry {
  return {
    id: entry.relativePath,
    hasChildren: entry.kind === "directory",
    sizeBytes: entry.kind === "file" ? 16 : null,
    modifiedAt: 1_000,
    createdAt: 500,
    ...entry,
  }
}

type BackendHandle = {
  backend: SlabWorkspaceBackendFileSystemProvider
  bridge: SlabWorkspaceBackendBridge
}

function createBackend(overrides: Partial<SlabWorkspaceBackendBridge> = {}): BackendHandle {
  const bridge: SlabWorkspaceBackendBridge = {
    readDirectory: vi.fn(async () => ({ relativePath: "", entries: [], truncated: false })),
    readFile: vi.fn(async (relativePath) => ({
      relativePath,
      name: relativePath.split("/").findLast(Boolean) ?? relativePath,
      content: "hello world",
      sizeBytes: 11,
      contentHash: "hash",
    })),
    statPath: vi.fn(async (relativePath) => ({
      relativePath,
      kind: "file" as const,
      sizeBytes: 11,
      modifiedAt: 1_000,
      createdAt: 500,
    })),
    writeFile: vi.fn(async () => ({ relativePath: "", sizeBytes: 0, contentHash: "" })),
    createDirectory: vi.fn(async () => ({ relativePath: "" })),
    renamePath: vi.fn(async () => ({ relativePath: "" })),
    deletePath: vi.fn(async () => ({ relativePath: "" })),
    watch: vi.fn(() => ({ dispose() {} })),
    ...overrides,
  }
  const backend = new SlabWorkspaceBackendFileSystemProvider({ bridge })
  backend.setWorkspaceRoot(WORKSPACE_ROOT)
  return { backend, bridge }
}

describe("SlabWorkspaceBackendFileSystemProvider", () => {
  it("stats the workspace root as a directory without a bridge call", async () => {
    const { backend, bridge } = createBackend()
    const stat = await backend.stat(resourceFor(""))
    expect(stat.type).toBe(FileType.Directory)
    expect(vi.mocked(bridge.statPath)).not.toHaveBeenCalled()
  })

  it("caches stat results so statPath is called once per path", async () => {
    const { backend, bridge } = createBackend()
    await backend.stat(resourceFor("src/a.ts"))
    await backend.stat(resourceFor("src/a.ts"))
    expect(vi.mocked(bridge.statPath)).toHaveBeenCalledTimes(1)
  })

  it("backfills stat from a directory listing without an extra statPath call", async () => {
    const { backend, bridge } = createBackend({
      readDirectory: vi.fn(async () => ({
        relativePath: "src",
        entries: [fileEntry({ kind: "file", name: "a.ts", relativePath: "src/a.ts" })],
        truncated: false,
      })),
    })
    const stat = await backend.stat(resourceFor("src/a.ts"))
    expect(stat.type).toBe(FileType.File)
    expect(vi.mocked(bridge.statPath)).not.toHaveBeenCalled()
  })

  it("throws when statPath rejects", async () => {
    const { backend } = createBackend({
      statPath: vi.fn(async () => {
        throw new Error("missing")
      }),
    })
    await expect(backend.stat(resourceFor("src/missing.ts"))).rejects.toThrow("not found")
  })

  it("rejects readFile for files above the preview size limit", async () => {
    const { backend } = createBackend({
      statPath: vi.fn(async (relativePath) => ({
        relativePath,
        kind: "file" as const,
        sizeBytes: 2 * 1024 * 1024,
        modifiedAt: 1_000,
        createdAt: 500,
      })),
    })
    await expect(backend.readFile(resourceFor("big.ts"))).rejects.toThrow(/too large/)
  })

  it("deduplicates concurrent reads of the same file", async () => {
    const { backend, bridge } = createBackend({
      readFile: vi.fn(async (relativePath: string) => ({
        relativePath,
        name: "a.ts",
        content: "content",
        sizeBytes: 7,
        contentHash: "hash",
      })),
    })
    await Promise.all([
      backend.readFile(resourceFor("src/a.ts")),
      backend.readFile(resourceFor("src/a.ts")),
    ])
    expect(vi.mocked(bridge.readFile)).toHaveBeenCalledTimes(1)
  })

  it("writes through the bridge and emits an UPDATED change for the resource", async () => {
    const { backend, bridge } = createBackend({
      writeFile: vi.fn(async () => ({ relativePath: "", sizeBytes: 0, contentHash: "" })),
    })
    const changes: { type: number; resource: URI }[][] = []
    backend.onDidChangeFile((events) => changes.push(events as never))

    await backend.writeFile(resourceFor("src/a.ts"), new TextEncoder().encode("new"))

    expect(vi.mocked(bridge.writeFile)).toHaveBeenCalledTimes(1)
    // writeFile invalidates the path (firing UPDATED for path + parent) and then
    // emits its own UPDATED, so flatten the batches and assert the resource is reported.
    const flattened = changes.flat()
    expect(flattened).toEqual(
      expect.arrayContaining([expect.objectContaining({ type: FileChangeType.UPDATED })]),
    )
    expect(
      flattened.some(
        (change) =>
          change.type === FileChangeType.UPDATED && change.resource.toString().includes("src/a.ts"),
      ),
    ).toBe(true)
  })

  it("emits ADDED on mkdir and DELETED on delete", async () => {
    const { backend } = createBackend({
      readDirectory: vi.fn(async () => ({
        relativePath: "",
        entries: [fileEntry({ kind: "directory", name: "src", relativePath: "src" })],
        truncated: false,
      })),
    })
    const changes: { type: number }[][] = []
    backend.onDidChangeFile((events) => changes.push(events as never))

    await backend.mkdir(resourceFor("src/new"))
    expect(changes.at(-1)?.[0]?.type).toBe(FileChangeType.ADDED)

    await backend.delete(resourceFor("src/new"), {
      recursive: false,
      useTrash: false,
      atomic: false,
    })
    expect(changes.at(-1)?.[0]?.type).toBe(FileChangeType.DELETED)
  })

  it("rejects operations on resources outside the workspace", async () => {
    const { backend } = createBackend()
    await expect(backend.stat(URI.parse("file:///c:/elsewhere/x.ts"))).rejects.toThrow(
      /outside the active workspace/,
    )
  })

  it("coalesces watch events into a single debounced change batch", async () => {
    vi.useFakeTimers()
    let watcher: { onEvent: (e: unknown) => void; onError: (e: unknown) => void } | null = null
    const { backend } = createBackend({
      watch: vi.fn(({ onEvent, onError }) => {
        watcher = { onEvent, onError }
        return { dispose() {} }
      }),
    })
    const changes: { type: number }[][] = []
    backend.onDidChangeFile((events) => changes.push(events as never))

    const disposable = backend.watch()
    expect(watcher).not.toBeNull()
    watcher!.onEvent({ sequenceNumber: 1, type: "created", relativePath: "src/a.ts", kind: "file" })
    watcher!.onEvent({ sequenceNumber: 2, type: "changed", relativePath: "src/b.ts", kind: "file" })
    // No flush yet (debounce window open).
    expect(changes).toHaveLength(0)

    await vi.advanceTimersByTimeAsync(1000)
    // After the debounce window both events flush together. The flush fires an
    // invalidation pass plus the explicit change batch, so both ADDED and
    // UPDATED appear across the emitted batches.
    const flattened = changes.flat()
    expect(flattened).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ type: FileChangeType.ADDED }),
        expect.objectContaining({ type: FileChangeType.UPDATED }),
      ]),
    )

    disposable.dispose()
    vi.useRealTimers()
  })

  it("shares a single backend watch across concurrent file-service watchers", () => {
    let disposeCount = 0
    const { backend, bridge } = createBackend({
      watch: vi.fn(() => ({
        dispose() {
          disposeCount += 1
        },
      })),
    })

    const first = backend.watch()
    const second = backend.watch()
    expect(vi.mocked(bridge.watch)).toHaveBeenCalledTimes(1)

    first.dispose()
    expect(disposeCount).toBe(0)

    second.dispose()
    expect(disposeCount).toBe(1)

    const third = backend.watch()
    expect(vi.mocked(bridge.watch)).toHaveBeenCalledTimes(2)
    third.dispose()
    expect(disposeCount).toBe(2)
  })

  it("clears the whole cache and signals a root update when the watch stream errors", async () => {
    let watcher: { onEvent: (e: unknown) => void; onError: (e: unknown) => void } | null = null
    const { backend } = createBackend({
      watch: vi.fn(({ onEvent, onError }) => {
        watcher = { onEvent, onError }
        return { dispose() {} }
      }),
    })
    const changes: { type: number; resource: URI }[][] = []
    backend.onDidChangeFile((events) => changes.push(events as never))

    const disposable = backend.watch()
    watcher!.onError(new Event("error"))

    expect(changes).toHaveLength(1)
    expect(changes[0]?.[0]?.resource.toString()).toBe(workspaceLspModelPath(WORKSPACE_ROOT, ""))
    disposable.dispose()
  })

  it("drops stale in-flight reads after clearCache bumps the generation", async () => {
    let firstCallTaken = false
    let resolveFirstStat: (value: WorkspacePathMetadata) => void = () => {}
    const { backend, bridge } = createBackend({
      statPath: vi.fn(() => {
        // First call is controllable so we can resolve it after clearCache;
        // later calls auto-resolve so the follow-up stat completes.
        if (!firstCallTaken) {
          firstCallTaken = true
          return new Promise<WorkspacePathMetadata>((resolve) => {
            resolveFirstStat = resolve
          })
        }
        return Promise.resolve({
          relativePath: "src/a.ts",
          kind: "file" as const,
          sizeBytes: 4,
          modifiedAt: 1,
          createdAt: 1,
        })
      }),
    })

    const pending = backend.stat(resourceFor("src/a.ts"))
    // stat awaits the directory backfill before it reaches statPath; wait until
    // statPath has actually been invoked.
    await vi.waitFor(() => {
      expect(vi.mocked(bridge.statPath)).toHaveBeenCalled()
    })
    backend.clearCache()
    resolveFirstStat({
      relativePath: "src/a.ts",
      kind: "file",
      sizeBytes: 4,
      modifiedAt: 1,
      createdAt: 1,
    })
    await pending

    // The cleared generation's result is not cached, so the next stat must hit the bridge again.
    expect(vi.mocked(bridge.statPath)).toHaveBeenCalledTimes(1)
    await backend.stat(resourceFor("src/a.ts"))
    expect(vi.mocked(bridge.statPath)).toHaveBeenCalledTimes(2)
  })
})

describe("SlabRemoteFileSystemProvider", () => {
  it("registers the backend delegate and exposes it via getBackend", () => {
    const overlay = new SlabRemoteFileSystemProvider()
    const { backend } = createBackend()
    const disposable = overlay.registerBackend(100, backend)

    expect(overlay.getBackend()).toBe(backend)
    disposable.dispose()
  })
})
