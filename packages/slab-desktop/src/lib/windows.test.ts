import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn<(command: string, args?: unknown) => Promise<unknown>>(),
}))
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn<() => { label: string }>(() => ({ label: "main" })),
}))

import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import {
  buildSurfaceLabel,
  closeSurfaceWindow,
  focusSurfaceWindow,
  getCurrentSurfaceContext,
  listSurfaceWindows,
  openSurfaceWindow,
  parseSurfaceLabel,
} from "./windows"

describe("surface window labels", () => {
  it("builds and parses surface labels (id may contain dashes)", () => {
    expect(buildSurfaceLabel("workspace", "task-7")).toBe("a2u-workspace-task-7")
    expect(parseSurfaceLabel("a2u-image-img-2")).toEqual({ surface: "image", id: "img-2" })
    expect(parseSurfaceLabel("a2u-plugin-team-plugin")).toEqual({
      surface: "plugin",
      id: "team-plugin",
    })
  })

  it("returns null for the main window, non-surface labels, and unknown kinds", () => {
    expect(parseSurfaceLabel("main")).toBeNull()
    expect(parseSurfaceLabel("plugin-video")).toBeNull()
    expect(parseSurfaceLabel("a2u-unknown-x")).toBeNull()
    expect(parseSurfaceLabel("a2u-onlyone")).toBeNull()
    expect(parseSurfaceLabel("a2u--id")).toBeNull()
  })
})

describe("surface window invoke wrappers", () => {
  beforeEach(() => vi.mocked(invoke).mockReset())

  it("openSurfaceWindow invokes the host command with the surface request", async () => {
    vi.mocked(invoke).mockResolvedValue({ label: "a2u-workspace-1", opened: true })
    const result = await openSurfaceWindow("workspace", "1")

    expect(result).toEqual({ label: "a2u-workspace-1", opened: true })
    expect(invoke).toHaveBeenCalledWith("open_surface_window", {
      request: { surface: "workspace", id: "1" },
    })
  })

  it("close / focus / list invoke their commands", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined)
    await closeSurfaceWindow("a2u-review-1")
    expect(invoke).toHaveBeenCalledWith("close_surface_window", { label: "a2u-review-1" })

    await focusSurfaceWindow("a2u-hub-1")
    expect(invoke).toHaveBeenCalledWith("focus_surface_window", { label: "a2u-hub-1" })

    vi.mocked(invoke).mockResolvedValue(["a2u-workspace-1"])
    const labels = await listSurfaceWindows()
    expect(labels).toEqual(["a2u-workspace-1"])
  })
})

describe("getCurrentSurfaceContext", () => {
  it("returns null for the main window", () => {
    vi.mocked(getCurrentWindow).mockReturnValue({ label: "main" } as never)
    expect(getCurrentSurfaceContext()).toBeNull()
  })

  it("returns the surface context for a surface window label", () => {
    vi.mocked(getCurrentWindow).mockReturnValue({ label: "a2u-plugin-team" } as never)
    expect(getCurrentSurfaceContext()).toEqual({ surface: "plugin", id: "team" })
  })
})
