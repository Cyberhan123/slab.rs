import { SLAB_THEME_TOKENS } from "@slab/plugin-sdk"

import type {
  PluginCallRequest,
  PluginCallResponse,
  PluginEventPayload,
  PluginInfo,
  PluginMountViewRequest,
  PluginMountViewResponse,
  PluginPickFileResponse,
  PluginThemeSnapshot,
  PluginViewBounds,
} from "../infra/tauri/plugin-host-bridge"

export type {
  PluginCallRequest,
  PluginCallResponse,
  PluginEventPayload,
  PluginInfo,
  PluginMountViewRequest,
  PluginMountViewResponse,
  PluginPickFileResponse,
  PluginThemeSnapshot,
  PluginViewBounds,
} from "../infra/tauri/plugin-host-bridge"

/**
 * Imperative access to the shell's plugin host (Tauri IPC on desktop).
 *
 * UI code goes through {@link getPluginHost} — never the concrete Tauri bridge
 * — so the web shells get the same degraded behavior the bridge itself used
 * outside Tauri (no-ops / empty lists / throwing on host-only actions).
 */
export interface PluginHostPort {
  runtimeList(): Promise<PluginInfo[]>
  mountView(request: PluginMountViewRequest): Promise<PluginMountViewResponse>
  updateViewBounds(request: { pluginId: string; bounds: PluginViewBounds }): Promise<void>
  unmountView(request: { pluginId: string }): Promise<void>
  call(request: PluginCallRequest): Promise<PluginCallResponse>
  pickFile(): Promise<PluginPickFileResponse>
  setThemeSnapshot(snapshot: PluginThemeSnapshot): Promise<void>
  themeSnapshot(): Promise<PluginThemeSnapshot | null>
  onEvent(
    pluginId: string,
    handler: (payload: PluginEventPayload) => void,
  ): Promise<() => void>
}

/** Web default: mirrors the bridge's behavior outside Tauri. */
const webPluginHost: PluginHostPort = {
  async runtimeList() {
    return []
  },
  async mountView() {
    throw new Error("plugin view mount is only available in Tauri mode")
  },
  async updateViewBounds() {},
  async unmountView() {},
  async call() {
    throw new Error("plugin call is only available in Tauri mode")
  },
  async pickFile() {
    throw new Error("plugin file picker is only available in Tauri mode")
  },
  async setThemeSnapshot() {},
  async themeSnapshot() {
    return null
  },
  async onEvent() {
    return () => {}
  },
}

let current: PluginHostPort = webPluginHost

/** Install the shell's plugin-host adapter. Call once at app assembly. */
export function setPluginHost(port: PluginHostPort): void {
  current = port
}

/** The currently installed plugin-host adapter (web no-op by default). */
export function getPluginHost(): PluginHostPort {
  return current
}

/**
 * Read the current theme tokens off the document root. DOM-based, so it lives
 * with the seam rather than the Tauri bridge.
 */
export function readPluginThemeSnapshot(
  targetDocument: Document = document,
): PluginThemeSnapshot {
  const root = targetDocument.documentElement;
  const computed = targetDocument.defaultView?.getComputedStyle(root);
  const tokens: Partial<Record<string, string>> = {};

  if (computed) {
    for (const token of SLAB_THEME_TOKENS) {
      const value = computed.getPropertyValue(`--${token}`).trim();
      if (value) {
        tokens[token] = value;
      }
    }
  }

  return {
    mode: root.classList.contains("dark") ? "dark" : "light",
    tokens,
    updatedAt: Date.now(),
  };
}
