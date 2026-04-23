import { SERVER_BASE_URL } from "@/lib/config";

export type PluginRecord = {
  id: string;
  name: string;
  version: string;
  valid: boolean;
  error?: string | null;
  manifestVersion: number;
  compatibility: Record<string, unknown> | null;
  uiEntry?: string | null;
  hasWasm: boolean;
  networkMode: string;
  allowHosts: string[];
  contributions: Record<string, unknown> | null;
  permissions: Record<string, unknown> | null;
  sourceKind: string;
  sourceRef?: string | null;
  installRoot?: string | null;
  installedVersion?: string | null;
  manifestHash?: string | null;
  enabled: boolean;
  runtimeStatus: string;
  lastError?: string | null;
  installedAt?: string | null;
  updatedAt?: string | null;
  lastSeenAt?: string | null;
  lastStartedAt?: string | null;
  lastStoppedAt?: string | null;
  availableVersion?: string | null;
  updateAvailable: boolean;
  removable: boolean;
};

export type PluginMarketRecord = {
  sourceId: string;
  id: string;
  name: string;
  version: string;
  description?: string | null;
  packageUrl: string;
  packageSha256?: string | null;
  homepage?: string | null;
  tags: string[];
  installedVersion?: string | null;
  enabled: boolean;
  updateAvailable: boolean;
};

export type InstallPluginPayload = {
  pluginId: string;
  sourceId?: string;
  version?: string;
  packageUrl?: string;
  packageSha256?: string;
};

type DeletePluginResponse = {
  id: string;
  deleted: boolean;
};

type StopPluginPayload = {
  lastError?: string | null;
};

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(new URL(path.replace(/^\//, ""), `${SERVER_BASE_URL}/`), {
    ...init,
    headers: {
      "content-type": "application/json",
      ...init?.headers,
    },
  });

  if (!response.ok) {
    let message = `Plugin API request failed with HTTP ${response.status}`;
    try {
      const error = await response.json();
      if (error && typeof error.message === "string") {
        message = error.message;
      }
    } catch {
      // ignore JSON parse failures
    }
    throw new Error(message);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

export function listPlugins(): Promise<PluginRecord[]> {
  return request<PluginRecord[]>("/v1/plugins");
}

export function getPlugin(pluginId: string): Promise<PluginRecord> {
  return request<PluginRecord>(`/v1/plugins/${encodeURIComponent(pluginId)}`);
}

export function listMarketPlugins(): Promise<PluginMarketRecord[]> {
  return request<PluginMarketRecord[]>("/v1/plugins/market");
}

export function installPlugin(payload: InstallPluginPayload): Promise<PluginRecord> {
  return request<PluginRecord>("/v1/plugins/install", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function enablePlugin(pluginId: string): Promise<PluginRecord> {
  return request<PluginRecord>(`/v1/plugins/${encodeURIComponent(pluginId)}/enable`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}

export function disablePlugin(pluginId: string): Promise<PluginRecord> {
  return request<PluginRecord>(`/v1/plugins/${encodeURIComponent(pluginId)}/disable`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}

export function startPlugin(pluginId: string): Promise<PluginRecord> {
  return request<PluginRecord>(`/v1/plugins/${encodeURIComponent(pluginId)}/start`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}

export function stopPlugin(
  pluginId: string,
  payload: StopPluginPayload = {},
): Promise<PluginRecord> {
  return request<PluginRecord>(`/v1/plugins/${encodeURIComponent(pluginId)}/stop`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function removePlugin(pluginId: string): Promise<DeletePluginResponse> {
  return request<DeletePluginResponse>(`/v1/plugins/${encodeURIComponent(pluginId)}`, {
    method: "DELETE",
  });
}

