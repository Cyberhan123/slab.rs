/**
 * Platform capability ports.
 *
 * These interfaces are the ONLY way `@slab/core` and `@slab/ui` code may reach
 * platform-specific behavior (Tauri IPC, native dialogs, the asset protocol…).
 * Concrete implementations live under `src/infra/{tauri,web,h5}` and are
 * injected by each shell at assembly time — never imported directly by
 * domain/application/UI code.
 */

/** Coarse runtime capabilities of the current shell. */
export interface SlabPlatformInfo {
  /** Running inside the Tauri desktop shell (native IPC available). */
  readonly desktop: boolean
  /** Running inside the mobile H5 shell. */
  readonly mobile: boolean
}

export interface FileDialogFilter {
  name: string
  extensions: string[]
}

export interface PickFileOptions {
  multiple?: boolean
  filters?: FileDialogFilter[]
}

/** Result of a native file pick: a local path (Tauri) or a web `File`. */
export interface PickedFile {
  /** Native filesystem path when picked via the host dialog. */
  path?: string
  /** Web `File` object when picked from an `<input type="file">`. */
  file?: File
  /** Best-effort display name for the picked entry. */
  name?: string
}

/**
 * Native folder/file picking. Web implementations return `null` so callers can
 * fall back to a manual path input or an `<input type="file">` element.
 */
export interface FileDialogPort {
  pickFolder(options?: { title?: string }): Promise<string | null>
  pickFile(options?: PickFileOptions): Promise<PickedFile | null>
  /** Multi-select variant; yields an empty array when nothing was picked. */
  pickFiles(options?: PickFileOptions): Promise<PickedFile[]>
}

/** Reading local media referenced by native paths. */
export interface MediaFilePort {
  /** Read a local file (native path) as a Blob. */
  readFile(path: string): Promise<Blob>
  /**
   * Stage recorded audio bytes as a host-side temp file and return its native
   * path (the transcription endpoint is path-only). Desktop shells only; web
   * implementations reject.
   */
  writeTempAudio(bytes: Uint8Array, extension: string): Promise<string>
  /** Best-effort removal of a staged temp audio file. */
  removeTempAudio(path: string): Promise<void>
}

/**
 * Resolve a local asset path into something the webview can render.
 *
 * - Tauri: `convertFileSrc(path)` (asset protocol)
 * - Web/H5: identity for URLs/data URLs, `null`-ish handling for local paths
 *   that only exist on the server (returned via `/v1/` URLs instead).
 */
export interface ImageSrcPort {
  resolve(pathOrUrl: string): string
  /** Whether local filesystem paths can be resolved at all on this platform. */
  canResolveLocalPaths(): boolean
}

/** Out-of-band error notification (toast on shells that have one). */
export interface NotificationPort {
  error(message: string, options?: { description?: string; id?: string }): void
}

/** Native window chrome controls (desktop shells only). */
export interface WindowChromePort {
  minimize(): Promise<void>
  toggleMaximize(): Promise<void>
  close(): Promise<void>
  /** Whether native window controls are available at all. */
  isAvailable(): boolean
}

/** Aggregate of every platform capability injected via `SlabProvider`. */
export type SlabPorts = {
  fileDialog: FileDialogPort
  mediaFile: MediaFilePort
  imageSrc: ImageSrcPort
  notifications: NotificationPort
  platformInfo: SlabPlatformInfo
  windowChrome: WindowChromePort
}
