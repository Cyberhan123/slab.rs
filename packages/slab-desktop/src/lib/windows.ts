/**
 * Independent a2u surface windows (TC-FE-06 / INFRA-11).
 *
 * Each a2u surface can open in its own OS window. The window **label**
 * (`a2u-<surface>-<id>`) is the source of truth for which surface a window
 * renders — the frontend reads its own label on load (see
 * {@link getCurrentSurfaceContext}) and self-routes, so no plugin/surface
 * payload can spoof the caller identity (AGENTS.md boundary).
 */
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"

export const SURFACE_WINDOW_PREFIX = "a2u-"

export type SurfaceKind = "workspace" | "image" | "review" | "plugin" | "hub"

const SURFACE_KINDS: readonly SurfaceKind[] = ["workspace", "image", "review", "plugin", "hub"]

export type SurfaceContext = { surface: SurfaceKind; id: string } | null

export type SurfaceWindowResponse = { label: string; opened: boolean }

function isSurfaceKind(value: string): value is SurfaceKind {
  return (SURFACE_KINDS as readonly string[]).includes(value)
}

/** Build the window label for a surface instance: `a2u-<surface>-<id>`. */
export function buildSurfaceLabel(surface: SurfaceKind, id: string): string {
  return `${SURFACE_WINDOW_PREFIX}${surface}-${id}`
}

/** Parse a window label into `{ surface, id }`, or `null` for the main window. */
export function parseSurfaceLabel(label: string): SurfaceContext {
  if (!label.startsWith(SURFACE_WINDOW_PREFIX)) {
    return null
  }
  const rest = label.slice(SURFACE_WINDOW_PREFIX.length)
  const dash = rest.indexOf("-")
  if (dash <= 0 || dash === rest.length - 1) {
    return null
  }
  const surface = rest.slice(0, dash)
  const id = rest.slice(dash + 1)
  if (!isSurfaceKind(surface)) {
    return null
  }
  return { surface, id }
}

/** Open (or focus) a dedicated OS window for a surface instance. */
export async function openSurfaceWindow(
  surface: SurfaceKind,
  id: string
): Promise<SurfaceWindowResponse> {
  return invoke<SurfaceWindowResponse>("open_surface_window", { request: { surface, id } })
}

/** Close a surface window by label. */
export async function closeSurfaceWindow(label: string): Promise<void> {
  await invoke<void>("close_surface_window", { label })
}

/** Focus a surface window by label. */
export async function focusSurfaceWindow(label: string): Promise<void> {
  await invoke<void>("focus_surface_window", { label })
}

/** List the labels of currently-open surface windows. */
export async function listSurfaceWindows(): Promise<string[]> {
  return invoke<string[]>("list_surface_windows")
}

/**
 * Read this window's label to determine which surface it renders. Returns
 * `null` for the main window (or any non-surface window).
 */
export function getCurrentSurfaceContext(): SurfaceContext {
  return parseSurfaceLabel(getCurrentWindow().label)
}
