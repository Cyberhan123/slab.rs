import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { isTauri } from "../../platform/detect";
import type {
  PluginCallRequest,
  PluginCallResponse,
  PluginEventPayload,
  PluginHostPort,
  PluginInfo,
  PluginMountViewRequest,
  PluginMountViewResponse,
  PluginPickFileResponse,
  PluginThemeSnapshot,
  PluginViewBounds,
} from "../../platform/plugin-host";

// The DTO types live with the platform seam (`platform/plugin-host.ts`); this
// bridge only implements the Tauri transport. Re-exported for any existing
// deep-path consumers.
export type {
  PluginCallRequest,
  PluginCallResponse,
  PluginCompatibility,
  PluginContributions,
  PluginEventPayload,
  PluginInfo,
  PluginMountViewRequest,
  PluginMountViewResponse,
  PluginPermissions,
  PluginPickFileResponse,
  PluginThemeSnapshot,
  PluginViewBounds,
} from "../../platform/plugin-host";

export async function pluginRuntimeList(): Promise<PluginInfo[]> {
  if (!isTauri()) return [];
  return invoke<PluginInfo[]>("plugin_list");
}

export async function pluginMountView(
  request: PluginMountViewRequest,
): Promise<PluginMountViewResponse> {
  if (!isTauri()) {
    throw new Error("plugin view mount is only available in Tauri mode");
  }
  return invoke<PluginMountViewResponse>("plugin_mount_view", { request });
}

export async function pluginUpdateViewBounds(request: {
  pluginId: string;
  bounds: PluginViewBounds;
}): Promise<void> {
  if (!isTauri()) return;
  await invoke("plugin_update_view_bounds", { request });
}

export async function pluginUnmountView(request: { pluginId: string }): Promise<void> {
  if (!isTauri()) return;
  await invoke("plugin_unmount_view", { request });
}

export async function pluginCall(request: PluginCallRequest): Promise<PluginCallResponse> {
  if (!isTauri()) {
    throw new Error("plugin call is only available in Tauri mode");
  }
  return invoke<PluginCallResponse>("plugin_call", { request });
}

export async function pluginPickFile(): Promise<PluginPickFileResponse> {
  if (!isTauri()) {
    throw new Error("plugin file picker is only available in Tauri mode");
  }
  return invoke<PluginPickFileResponse>("plugin_pick_file");
}

export async function pluginSetThemeSnapshot(
  snapshot: PluginThemeSnapshot,
): Promise<void> {
  if (!isTauri()) return;
  await invoke("plugin_set_theme_snapshot", { snapshot });
}

export async function pluginThemeSnapshot(): Promise<PluginThemeSnapshot | null> {
  if (!isTauri()) return null;
  return invoke<PluginThemeSnapshot>("plugin_theme_snapshot");
}

export async function pluginOnEvent(
  pluginId: string,
  handler: (payload: PluginEventPayload) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => {};
  }

  const eventName = `plugin://${pluginId}/event`;
  return listen<PluginEventPayload>(eventName, (event) => {
    handler(event.payload);
  });
}


/** Tauri-backed {@link PluginHostPort}; degrades to web behavior outside Tauri. */
export const tauriPluginHost: PluginHostPort = {
  runtimeList: pluginRuntimeList,
  mountView: pluginMountView,
  updateViewBounds: pluginUpdateViewBounds,
  unmountView: pluginUnmountView,
  call: pluginCall,
  pickFile: pluginPickFile,
  setThemeSnapshot: pluginSetThemeSnapshot,
  themeSnapshot: pluginThemeSnapshot,
  onEvent: pluginOnEvent,
};
