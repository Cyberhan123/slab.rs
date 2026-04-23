import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  SLAB_THEME_TOKENS,
  type SlabThemeSnapshot,
  type SlabThemeTokenName,
} from "@slab/plugin-sdk";
import { isTauri } from "@/hooks/use-tauri";

export type PluginInfo = {
  id: string;
  name: string;
  version: string;
  valid: boolean;
  error?: string | null;
  manifestVersion: number;
  compatibility: PluginCompatibility;
  uiEntry?: string | null;
  hasWasm?: boolean;
  networkMode: "blocked" | "allowlist" | string;
  allowHosts: string[];
  contributions: PluginContributions;
  permissions: PluginPermissions;
};

export type PluginCompatibility = {
  slab?: string | null;
  pluginApi?: string | null;
};

export type PluginContributions = {
  routes: PluginRouteContribution[];
  sidebar: PluginSidebarContribution[];
  commands: PluginCommandContribution[];
  settings: PluginSettingsContribution[];
  agentCapabilities: PluginAgentCapabilityContribution[];
};

export type PluginRouteContribution = {
  id: string;
  path: string;
  title?: string | null;
  titleKey?: string | null;
  entry?: string | null;
};

export type PluginSidebarContribution = {
  id: string;
  label?: string | null;
  labelKey?: string | null;
  route?: string | null;
  command?: string | null;
  icon?: string | null;
};

export type PluginCommandContribution = {
  id: string;
  label?: string | null;
  labelKey?: string | null;
  action?: string | null;
  route?: string | null;
};

export type PluginSettingsContribution = {
  id: string;
  title?: string | null;
  titleKey?: string | null;
  schema: string;
};

export type PluginAgentCapabilityContribution = {
  id: string;
  kind: "tool" | "workflow" | string;
  description?: string | null;
  descriptionKey?: string | null;
  inputSchema?: string | null;
  outputSchema?: string | null;
  effects: string[];
  transport: {
    type: "pluginCall" | string;
    function: string;
  };
  exposeAsMcpTool: boolean;
};

export type PluginPermissions = {
  network: {
    mode: "blocked" | "allowlist" | string;
    allowHosts: string[];
  };
  ui: string[];
  agent: string[];
  slabApi: string[];
  files: {
    read: string[];
    write: string[];
  };
};

export type PluginViewBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type PluginMountViewRequest = {
  pluginId: string;
  bounds: PluginViewBounds;
};

export type PluginMountViewResponse = {
  pluginId: string;
  webviewLabel: string;
  url: string;
};

export type PluginCallRequest = {
  pluginId: string;
  function: string;
  input?: string;
};

export type PluginCallResponse = {
  outputText: string;
  outputBase64: string;
};

export type PluginApiRequest = {
  method: string;
  path: string;
  headers?: Record<string, string>;
  body?: string | null;
  timeoutMs?: number | null;
};

export type PluginApiResponse = {
  status: number;
  headers: Record<string, string>;
  body: string;
};

export type PluginPickFileResponse = {
  path: string | null;
};

export type PluginEventPayload = {
  pluginId: string;
  topic: string;
  data: unknown;
  ts: number;
};

export type PluginThemeSnapshot = SlabThemeSnapshot;

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

export async function pluginApiRequest(
  request: PluginApiRequest,
): Promise<PluginApiResponse> {
  if (!isTauri()) {
    throw new Error("plugin api request is only available in Tauri mode");
  }
  return invoke<PluginApiResponse>("plugin_api_request", { request });
}

export function readPluginThemeSnapshot(
  targetDocument: Document = document,
): PluginThemeSnapshot {
  const root = targetDocument.documentElement;
  const computed = targetDocument.defaultView?.getComputedStyle(root);
  const tokens: Partial<Record<SlabThemeTokenName, string>> = {};

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

