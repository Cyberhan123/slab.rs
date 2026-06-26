import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri"
import { Emitter } from "@codingame/monaco-vscode-api/vscode/vs/base/common/event"
import type { IDisposable } from "@codingame/monaco-vscode-api/vscode/vs/base/common/lifecycle"
import {
  FileChangeType,
  FileSystemProviderCapabilities,
  FileSystemProviderError,
  FileSystemProviderErrorCode,
  FileType,
  OverlayFileSystemProvider,
  registerFileSystemOverlay,
  type IFileChange,
  type IFileDeleteOptions,
  type IStat,
  type IFileSystemProviderWithFileReadWriteCapability,
} from "@codingame/monaco-vscode-files-service-override"
import {
  workspaceCreateDirectory,
  workspaceDeletePath,
  workspaceReadDirectory,
  workspaceReadFile,
  workspaceRenamePath,
  workspaceStatPath,
  workspaceWatch,
  workspaceWriteFile,
} from "@/lib/workspace-bridge"
import type {
  WorkspaceDirectoryResponse,
  WorkspaceFileContent,
  WorkspaceWatchEvent,
} from "@/lib/workspace-bridge"
import {
  workspaceLspFileUri,
  workspaceLspModelPath,
  workspaceLspRelativePathFromUri,
} from "./workspace-uri"

const noopDisposable: IDisposable = { dispose() {} }

// Files larger than this are rejected by readFile to avoid loading huge blobs into the editor.
const MAX_WORKSPACE_PREVIEW_BYTES = 1024 * 1024
// Fetch the whole workspace tree in ONE deep request when a workspace opens, then serve the
// explorer's per-folder `readdir`/`stat` calls entirely from the in-memory cache. The monaco
// explorer is chatty (a `readdir` per folder plus single-child-chain resolution and re-resolves),
// so lazy-loading folder-by-folder meant hundreds of `/v1/workspace/directory` round trips and
// frontend timeouts. Keep this in lockstep with the backend MAX_DIRECTORY_DEPTH cap
// (crates/slab-app-core/.../workspace/mod.rs); 128 is effectively unbounded for real trees.
const WORKSPACE_PRELOAD_DEPTH = 128
// Watch events arrive in bursts (a single save can emit several); coalesce them into one
// targeted cache invalidation pass.
const WORKSPACE_INVALIDATE_DEBOUNCE_MS = 100

/**
 * Bridge between the file-system provider and the workspace HTTP API. Defaults to the real
 * `workspace-bridge` functions; tests inject a fake so the delegate is unit-testable without
 * mocking the API client module.
 */
export interface SlabWorkspaceBackendBridge {
  readDirectory: typeof workspaceReadDirectory
  readFile: typeof workspaceReadFile
  statPath: typeof workspaceStatPath
  writeFile: typeof workspaceWriteFile
  createDirectory: typeof workspaceCreateDirectory
  renamePath: typeof workspaceRenamePath
  deletePath: typeof workspaceDeletePath
  watch: typeof workspaceWatch
}

const defaultSlabWorkspaceBackendBridge: SlabWorkspaceBackendBridge = {
  readDirectory: workspaceReadDirectory,
  readFile: workspaceReadFile,
  statPath: workspaceStatPath,
  writeFile: workspaceWriteFile,
  createDirectory: workspaceCreateDirectory,
  renamePath: workspaceRenamePath,
  deletePath: workspaceDeletePath,
  watch: workspaceWatch,
}

export type SlabWorkspaceBackendDebugHooks = {
  pushDirectory?: (entry: unknown) => void
  pushStat?: (entry: unknown) => void
}

export type SlabWorkspaceBackendFileSystemProviderOptions = {
  bridge?: SlabWorkspaceBackendBridge
  debug?: SlabWorkspaceBackendDebugHooks
}

type SharedWorkspaceWatch = {
  dispose: () => void
  flushTimer: ReturnType<typeof setTimeout> | null
  pendingEvents: WorkspaceWatchEvent[]
  refCount: number
  root: string
}

/**
 * Backs the workspace `file:` scheme by reading/writing the remote workspace API. Holds the
 * multi-layer cache (directory listings, path stats, in-flight deduplication), the depth-2
 * preload, watch-event debouncing, and change-event emission. Registered as a delegate into
 * {@link SlabRemoteFileSystemProvider}.
 */
export class SlabWorkspaceBackendFileSystemProvider
  implements IFileSystemProviderWithFileReadWriteCapability
{
  readonly capabilities = FileSystemProviderCapabilities.FileReadWrite
  readonly onDidChangeCapabilities = () => noopDisposable

  private readonly bridge: SlabWorkspaceBackendBridge
  private readonly debug: SlabWorkspaceBackendDebugHooks
  private readonly changesEmitter = new Emitter<readonly IFileChange[]>()
  readonly onDidChangeFile = this.changesEmitter.event

  private readonly textEncoder = new TextEncoder()
  private readonly textDecoder = new TextDecoder()

  private workspaceRoot: string | null = null

  private readonly directoryCache = new Map<string, WorkspaceDirectoryResponse>()
  private readonly pendingDirectoryReads = new Map<
    string,
    { generation: number; promise: Promise<WorkspaceDirectoryResponse> }
  >()
  private readonly pendingFileReads = new Map<string, Promise<WorkspaceFileContent>>()
  private readonly pathStatCache = new Map<string, IStat>()
  private readonly pendingPathStats = new Map<
    string,
    { generation: number; promise: Promise<IStat> }
  >()
  private sharedWatch: SharedWorkspaceWatch | null = null
  private cacheGeneration = 0
  // The workspace root is a synthetic directory: pin a stable ctime/mtime so repeated
  // stat() calls return identical values. A drifting mtime (Date.now()) makes the file
  // service treat the root as perpetually changed, which re-resolves the explorer tree
  // mid-expansion and drops lazy-loaded children.
  private rootStatTimestamp = 0

  constructor(options: SlabWorkspaceBackendFileSystemProviderOptions = {}) {
    this.bridge = options.bridge ?? defaultSlabWorkspaceBackendBridge
    this.debug = options.debug ?? {}
  }

  setWorkspaceRoot(root: string | null): void {
    this.workspaceRoot = root
  }

  /** Drops every cached listing/stat and bumps the generation so in-flight reads can no
   * longer write back into the cache. Used on workspace switch and on watch-stream loss. */
  clearCache(): void {
    this.cacheGeneration += 1
    this.directoryCache.clear()
    this.pendingDirectoryReads.clear()
    this.pendingFileReads.clear()
    this.pathStatCache.clear()
    this.pendingPathStats.clear()
    this.rootStatTimestamp = 0
    this.stopSharedWatch()
  }

  /** Pre-loads the first directory levels in one request so the explorer renders without
   * re-fetching each folder as it expands. Safe to call before `setWorkspaceRoot`. */
  async preload(root: string): Promise<void> {
    if (!root) {
      return
    }
    let flat: WorkspaceDirectoryResponse
    try {
      flat = await this.bridge.readDirectory("", { depth: WORKSPACE_PRELOAD_DEPTH })
    } catch {
      return
    }
    // A rapid workspace switch could change the root while the fetch was in flight.
    if (this.workspaceRoot !== root) {
      return
    }

    // Group every entry under its parent directory so readdir()/stat() for any pre-loaded
    // level resolve from cache. Directories at the depth boundary are listed as entries but
    // their own children were not fetched, so they have no directoryCache entry and still
    // lazy-load on expand.
    const entriesByParent = new Map<string, WorkspaceDirectoryResponse["entries"]>()
    const now = Date.now()
    for (const entry of flat.entries) {
      const separatorIndex = entry.relativePath.lastIndexOf("/")
      const parent = separatorIndex === -1 ? "" : entry.relativePath.slice(0, separatorIndex)
      let siblings = entriesByParent.get(parent)
      if (!siblings) {
        siblings = []
        entriesByParent.set(parent, siblings)
      }
      siblings.push(entry)
      this.pathStatCache.set(entry.relativePath, {
        ctime: entry.createdAt ?? now,
        mtime: entry.modifiedAt ?? now,
        size: entry.kind === "file" ? entry.sizeBytes ?? 0 : 0,
        type: entry.kind === "directory" ? FileType.Directory : FileType.File,
      })
    }
    for (const [parent, entries] of entriesByParent) {
      // Never clobber a fresher in-flight readdir for this directory.
      if (!this.directoryCache.has(parent)) {
        this.directoryCache.set(parent, {
          relativePath: parent,
          entries,
          truncated: parent === "" ? flat.truncated : false,
        })
      }
    }
    this.pathStatCache.set("", { ctime: now, mtime: now, size: 0, type: FileType.Directory })
  }

  async stat(resource: URI): Promise<IStat> {
    const relativePath = this.relativePathForResource(resource.toString())
    if (!relativePath) {
      this.rootStatTimestamp ||= Date.now()
      this.debug.pushStat?.({ relativePath, result: "root" })
      return {
        ctime: this.rootStatTimestamp,
        mtime: this.rootStatTimestamp,
        size: 0,
        type: FileType.Directory,
      }
    }

    try {
      const cachedStat = this.pathStatCache.get(relativePath)
      if (cachedStat) {
        this.debug.pushStat?.({ relativePath, result: "cache", type: cachedStat.type })
        return cachedStat
      }

      const separatorIndex = relativePath.lastIndexOf("/")
      const parentRelativePath = separatorIndex === -1 ? "" : relativePath.slice(0, separatorIndex)
      await this.loadWorkspaceDirectory(parentRelativePath).catch(() => null)
      const directoryBackedStat = this.pathStatCache.get(relativePath)
      if (directoryBackedStat) {
        this.debug.pushStat?.({
          relativePath,
          result: "directory-cache",
          type: directoryBackedStat.type,
        })
        return directoryBackedStat
      }

      const generation = this.cacheGeneration
      const pendingStat = this.pendingPathStats.get(relativePath)
      if (pendingStat?.generation === generation) {
        this.debug.pushStat?.({ relativePath, result: "pending" })
        return await pendingStat.promise
      }

      const metadataPromise = this.bridge
        .statPath(relativePath)
        .then((metadata) => {
          const nextStat: IStat = {
            ctime: metadata.createdAt || Date.now(),
            mtime: metadata.modifiedAt || Date.now(),
            size: metadata.sizeBytes,
            type: metadata.kind === "directory" ? FileType.Directory : FileType.File,
          }

          if (generation === this.cacheGeneration) {
            this.pathStatCache.set(relativePath, nextStat)
          }

          this.debug.pushStat?.({ relativePath, result: "fetch", type: nextStat.type })
          return nextStat
        })
        .finally(() => {
          const currentPendingStat = this.pendingPathStats.get(relativePath)
          if (
            currentPendingStat?.generation === generation &&
            currentPendingStat.promise === metadataPromise
          ) {
            this.pendingPathStats.delete(relativePath)
          }
        })

      this.pendingPathStats.set(relativePath, { generation, promise: metadataPromise })
      return await metadataPromise
    } catch {
      this.debug.pushStat?.({ relativePath, result: "not-found" })
      throw FileSystemProviderError.create(
        "workspace LSP file was not found",
        FileSystemProviderErrorCode.FileNotFound,
      )
    }
  }

  async readFile(resource: URI): Promise<Uint8Array> {
    const relativePath = this.relativePathForResource(resource.toString())
    this.debug.pushStat?.({ relativePath, result: "read-file-start" })
    const pendingFile = this.pendingFileReads.get(relativePath)
    const filePromise =
      pendingFile ??
      (async () => {
        const metadata = await this.bridge.statPath(relativePath)
        if (metadata.sizeBytes > MAX_WORKSPACE_PREVIEW_BYTES) {
          throw FileSystemProviderError.create(
            `workspace file is too large to preview (${metadata.sizeBytes} bytes; maximum is ${MAX_WORKSPACE_PREVIEW_BYTES} bytes)`,
            FileSystemProviderErrorCode.Unavailable,
          )
        }
        const nextFile = await this.bridge.readFile(relativePath)
        return nextFile
      })().finally(() => {
        this.pendingFileReads.delete(relativePath)
      })

    if (!pendingFile) {
      this.pendingFileReads.set(relativePath, filePromise)
    }
    let file: WorkspaceFileContent
    try {
      file = await filePromise
      this.debug.pushStat?.({
        bytes: file.content.length,
        relativePath,
        result: pendingFile ? "read-file-pending" : "read-file-fetch",
      })
    } catch (error) {
      this.debug.pushStat?.({
        error: error instanceof Error ? error.message : String(error),
        relativePath,
        result: "read-file-error",
      })
      throw error
    }

    return this.textEncoder.encode(file.content)
  }

  async readdir(resource: URI): Promise<[string, FileType][]> {
    const relativePath = this.relativePathForResource(resource.toString())
    const directory = await this.loadWorkspaceDirectory(relativePath)

    return directory.entries.map(
      (entry): [string, FileType] => [
        entry.name,
        entry.kind === "directory" ? FileType.Directory : FileType.File,
      ],
    )
  }

  async writeFile(resource: URI, content: Uint8Array): Promise<void> {
    const relativePath = this.relativePathForResource(resource.toString())
    await this.bridge.writeFile({
      content: this.textDecoder.decode(content),
      relativePath,
    })
    this.invalidateWorkspacePaths([relativePath])
    this.changesEmitter.fire([{ resource, type: FileChangeType.UPDATED }])
  }

  async mkdir(resource: URI): Promise<void> {
    const relativePath = this.relativePathForResource(resource.toString())
    await this.bridge.createDirectory({ relativePath })
    this.invalidateWorkspacePaths([relativePath])
    this.changesEmitter.fire([{ resource, type: FileChangeType.ADDED }])
  }

  async delete(resource: URI, options: IFileDeleteOptions): Promise<void> {
    const relativePath = this.relativePathForResource(resource.toString())
    await this.bridge.deletePath({
      recursive: Boolean(options.recursive),
      relativePath,
    })
    this.invalidateWorkspacePaths([relativePath])
    this.changesEmitter.fire([{ resource, type: FileChangeType.DELETED }])
  }

  async rename(from: URI, to: URI): Promise<void> {
    const fromRelativePath = this.relativePathForResource(from.toString())
    const toRelativePath = this.relativePathForResource(to.toString())
    await this.bridge.renamePath({ fromRelativePath, toRelativePath })
    this.invalidateWorkspacePaths([fromRelativePath, toRelativePath])
    this.changesEmitter.fire([
      { resource: from, type: FileChangeType.DELETED },
      { resource: to, type: FileChangeType.ADDED },
    ])
  }

  watch(): IDisposable {
    const activeWorkspaceRoot = this.workspaceRoot
    if (!activeWorkspaceRoot) {
      return noopDisposable
    }

    if (this.sharedWatch?.root === activeWorkspaceRoot) {
      const watch = this.sharedWatch
      watch.refCount += 1
      this.debug.pushStat?.({
        refCount: watch.refCount,
        relativePath: "",
        result: "watch-reuse",
      })
      let disposed = false
      return {
        dispose: () => {
          if (disposed) {
            return
          }
          disposed = true
          this.releaseSharedWatch(watch)
        },
      }
    }

    this.stopSharedWatch()
    const watch: SharedWorkspaceWatch = {
      dispose: () => {},
      flushTimer: null,
      pendingEvents: [],
      refCount: 1,
      root: activeWorkspaceRoot,
    }
    this.sharedWatch = watch
    this.debug.pushStat?.({ relativePath: "", result: "watch-start" })
    const flushPendingEvents = () => {
      watch.flushTimer = null
      if (watch.pendingEvents.length === 0) {
        return
      }
      const events = watch.pendingEvents
      watch.pendingEvents = []
      // Drop the cached listings/stats for the affected paths (and their parents, since
      // directory membership changed) so the next access re-fetches just those, instead of
      // nuking the whole cache.
      this.invalidateWorkspacePaths(events.map((event) => event.relativePath))
      this.changesEmitter.fire(
        events.map((event) => ({
          resource: URI.parse(workspaceLspModelPath(activeWorkspaceRoot, event.relativePath)),
          type:
            event.type === "created"
              ? FileChangeType.ADDED
              : event.type === "deleted"
                ? FileChangeType.DELETED
                : FileChangeType.UPDATED,
        })),
      )
    }

    const watchDisposable = this.bridge.watch({
      onError: () => {
        // The SSE stream was lost, so we cannot trust the cache to reflect out-of-band
        // changes. Conservatively reset everything and signal a root update so the explorer
        // re-reads on next access.
        this.clearCache()
        this.changesEmitter.fire([
          {
            resource: URI.parse(workspaceLspFileUri(activeWorkspaceRoot)),
            type: FileChangeType.UPDATED,
          },
        ])
      },
      onEvent: (event) => {
        watch.pendingEvents.push(event)
        if (watch.flushTimer === null) {
          watch.flushTimer = setTimeout(flushPendingEvents, WORKSPACE_INVALIDATE_DEBOUNCE_MS)
        }
      },
    })
    watch.dispose = () => {
      if (watch.flushTimer !== null) {
        clearTimeout(watch.flushTimer)
        watch.flushTimer = null
      }
      watch.pendingEvents = []
      watchDisposable.dispose()
    }

    let disposed = false
    return {
      dispose: () => {
        if (disposed) {
          return
        }
        disposed = true
        this.releaseSharedWatch(watch)
      },
    }
  }

  private releaseSharedWatch(watch: SharedWorkspaceWatch): void {
    if (this.sharedWatch !== watch) {
      return
    }
    watch.refCount -= 1
    this.debug.pushStat?.({
      refCount: watch.refCount,
      relativePath: "",
      result: "watch-release",
    })
    if (watch.refCount <= 0) {
      this.stopSharedWatch()
    }
  }

  private stopSharedWatch(): void {
    if (!this.sharedWatch) {
      return
    }
    const watch = this.sharedWatch
    this.sharedWatch = null
    this.debug.pushStat?.({ relativePath: "", result: "watch-stop" })
    watch.dispose()
  }

  private invalidateWorkspacePaths(relativePaths: string[]): void {
    if (relativePaths.length === 0) {
      return
    }
    const touched = new Set<string>()
    for (const relativePath of relativePaths) {
      const separatorIndex = relativePath.lastIndexOf("/")
      const parent = separatorIndex === -1 ? "" : relativePath.slice(0, separatorIndex)
      for (const key of [relativePath, parent]) {
        touched.add(key)
        this.directoryCache.delete(key)
        this.pendingDirectoryReads.delete(key)
        this.pathStatCache.delete(key)
        this.pendingPathStats.delete(key)
      }
    }
    const activeWorkspaceRoot = this.workspaceRoot
    if (activeWorkspaceRoot) {
      this.changesEmitter.fire(
        [...touched].map((relativePath) => ({
          resource: URI.parse(workspaceLspModelPath(activeWorkspaceRoot, relativePath)),
          type: FileChangeType.UPDATED,
        })),
      )
    }
  }

  private async loadWorkspaceDirectory(relativePath: string): Promise<WorkspaceDirectoryResponse> {
    const cachedDirectory = this.directoryCache.get(relativePath)
    if (cachedDirectory) {
      this.debug.pushDirectory?.({
        cached: true,
        entries: cachedDirectory.entries.map((entry) => entry.relativePath),
        relativePath,
      })
      return cachedDirectory
    }

    const generation = this.cacheGeneration
    const pendingDirectory = this.pendingDirectoryReads.get(relativePath)
    if (pendingDirectory?.generation === generation) {
      return pendingDirectory.promise
    }

    const directoryPromise = this.bridge
      .readDirectory(relativePath)
      .then((nextDirectory) => {
        this.debug.pushDirectory?.({
          cached: false,
          entries: nextDirectory.entries.map((entry) => entry.relativePath),
          relativePath,
        })
        if (generation === this.cacheGeneration) {
          this.directoryCache.set(relativePath, nextDirectory)
          const now = Date.now()
          for (const entry of nextDirectory.entries) {
            this.pathStatCache.set(entry.relativePath, {
              ctime: entry.createdAt ?? now,
              mtime: entry.modifiedAt ?? now,
              size: entry.kind === "file" ? entry.sizeBytes ?? 0 : 0,
              type: entry.kind === "directory" ? FileType.Directory : FileType.File,
            })
          }
          // Deliberately do NOT fire onDidChangeFile here: a directory read is not a
          // change. Firing UPDATED on every readdir makes the explorer refresh the
          // folder mid-expansion, which drops its just-loaded children (deep trees
          // never stabilize). The explorer renders readdir results directly; real
          // mutations are surfaced via writeFile/mkdir/delete/rename and the watcher.
        }

        return nextDirectory
      })
      .finally(() => {
        const currentPendingDirectory = this.pendingDirectoryReads.get(relativePath)
        if (
          currentPendingDirectory?.generation === generation &&
          currentPendingDirectory.promise === directoryPromise
        ) {
          this.pendingDirectoryReads.delete(relativePath)
        }
      })

    this.pendingDirectoryReads.set(relativePath, { generation, promise: directoryPromise })
    return directoryPromise
  }

  private relativePathForResource(resource: string): string {
    const workspaceRoot = this.workspaceRoot
    const relativePath = workspaceRoot
      ? workspaceLspRelativePathFromUri(workspaceRoot, resource)
      : null
    if (relativePath === null) {
      throw FileSystemProviderError.create(
        "workspace LSP file is outside the active workspace",
        FileSystemProviderErrorCode.NoPermissions,
      )
    }

    return relativePath
  }
}

/**
 * The slab-owned `file:` overlay. Extends the upstream {@link OverlayFileSystemProvider} so we
 * inherit the priority-based delegate machinery (`register`/`delegates`/`readFromDelegates`/
 * `writeToDelegates`), capability aggregation, and change-event fan-out. Additional delegates
 * (e.g. memory-file overlays) can be registered by priority without calling
 * `registerFileSystemOverlay` again.
 */
export class SlabRemoteFileSystemProvider extends OverlayFileSystemProvider {
  private backend: SlabWorkspaceBackendFileSystemProvider | null = null

  /** Registers the remote-backend delegate at a priority and keeps a typed handle to it. */
  registerBackend(
    priority: number,
    provider: SlabWorkspaceBackendFileSystemProvider,
  ): IDisposable {
    this.backend = provider
    return super.register(priority, provider)
  }

  getBackend(): SlabWorkspaceBackendFileSystemProvider | null {
    return this.backend
  }
}

let slabRemoteFileSystemProvider: SlabRemoteFileSystemProvider | null = null
let slabWorkspaceBackend: SlabWorkspaceBackendFileSystemProvider | null = null
let slabFileSystemOverlayRegistered = false
// Remembered so the backend picks up the active root when it is created. The backend is not
// created here to avoid registering the overlay before the VS Code service overrides are up.
let pendingWorkspaceRoot: string | null = null

/**
 * Creates the slab overlay and its remote-backend delegate once, registers the backend into the
 * overlay, and registers the overlay into the @codingame `file:` overlay above the default
 * in-memory provider. Idempotent. Returns the singleton overlay and backend.
 */
export function ensureSlabWorkspaceFileSystem(options?: {
  backend?: SlabWorkspaceBackendFileSystemProviderOptions
}): {
  overlay: SlabRemoteFileSystemProvider
  backend: SlabWorkspaceBackendFileSystemProvider
} {
  if (!slabRemoteFileSystemProvider) {
    slabRemoteFileSystemProvider = new SlabRemoteFileSystemProvider()
    slabWorkspaceBackend = new SlabWorkspaceBackendFileSystemProvider(options?.backend)
    slabWorkspaceBackend.setWorkspaceRoot(pendingWorkspaceRoot)
    // The backend is the overlay's sole high-priority delegate. The inner priority only
    // matters once additional delegates are registered.
    slabRemoteFileSystemProvider.registerBackend(100, slabWorkspaceBackend)
  }
  if (!slabFileSystemOverlayRegistered) {
    // Priority 1 places the whole slab overlay in front of the default in-memory provider
    // (priority 0), so reads/writes resolve against the remote workspace backend first.
    registerFileSystemOverlay(1, slabRemoteFileSystemProvider)
    slabFileSystemOverlayRegistered = true
  }
  return {
    overlay: slabRemoteFileSystemProvider,
    backend: slabWorkspaceBackend!,
  }
}

export function setSlabWorkspaceFileSystemRoot(root: string | null): void {
  pendingWorkspaceRoot = root
  slabWorkspaceBackend?.setWorkspaceRoot(root)
}

export function clearSlabWorkspaceFileSystemCache(): void {
  slabWorkspaceBackend?.clearCache()
}

export async function preloadSlabWorkspaceFileSystem(root: string): Promise<void> {
  if (!slabWorkspaceBackend) {
    // Matches the pre-registration no-op semantics of the old module-level binding.
    return
  }
  await slabWorkspaceBackend.preload(root)
}
