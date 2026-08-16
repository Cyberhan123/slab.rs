import { SLAB_THEME_TOKENS, type SlabThemeSnapshot } from "@slab/plugin-sdk"

// ── Plugin host DTOs (owned here; the Tauri bridge consumes them) ───────────

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
  agentHooks: PluginAgentHookContribution[];
  languageServers: PluginLanguageServerContribution[];
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

export type PluginAgentHookContribution = {
  id: string;
  description?: string | null;
  descriptionKey?: string | null;
  events: Array<
    | "on_agent_start"
    | "on_llm_start"
    | "on_llm_end"
    | "on_tool_start"
    | "on_tool_end"
    | "on_agent_end"
    | string
  >;
  transport: {
    runtime: "javascript" | "python" | string;
    function: string;
  };
};

export type PluginLanguageServerContribution = {
  id: string;
  languages: string[];
  transport:
    | {
        type: "stdio";
        command: string;
        args?: string[];
        env?: Record<string, string>;
      }
    | {
        type: "webSocket";
        url: string;
      }
    | {
        /** npm package bundled inside the plugin directory. The runtime resolves
         *  the command from the plugin's node_modules/.bin/ before falling back
         *  to the system PATH, so no global installation is required. */
        type: "nodePackage";
        package: string;
        command?: string;
        args?: string[];
        env?: Record<string, string>;
      };
};

export type PluginPermissions = {
  network: {
    mode: "blocked" | "allowlist" | string;
    allowHosts: string[];
  };
  ui: string[];
  agent: string[];
  lsp: string[];
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

// ── Plugin host seam ────────────────────────────────────────────────────────

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
