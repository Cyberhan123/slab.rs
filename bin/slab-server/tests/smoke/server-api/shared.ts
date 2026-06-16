import type { components, paths } from "@slab/api";
import { createHash } from "node:crypto";
import { expect } from "vitest";

import type { JsonResponse, SlabServerTestHarness } from "../../support/slab-server";

export type ApiPath = keyof paths;
export type HttpMethod = "delete" | "get" | "post" | "put";
export type OpenApiDocument = {
  openapi?: string;
  paths?: Record<string, Partial<Record<HttpMethod, unknown>>>;
};
export type HealthResponse = {
  status?: string;
  version?: string;
};
export type ServerErrorResponse = {
  code?: number;
  data?: unknown;
  message?: string;
};
export type OpenAiErrorResponse = {
  error?: {
    code?: string | null;
    message?: string;
    param?: string | null;
    type?: string;
  };
};
export type DeletedModelResponse = {
  id: string;
  status: string;
};
export type Schema = components["schemas"];
export type TaskResponse = Schema["TaskResponse"];
export type SmokeOperation = {
  method: HttpMethod;
  path: ApiPath;
};

export const externalBaseUrl = process.env.SLAB_SERVER_BASE_URL?.trim();
export const jsonHeaders = {
  "Content-Type": "application/json"
};
const documentedMethods: readonly HttpMethod[] = ["delete", "get", "post", "put"];

export const executableSmokeOperations = [
  { method: "get", path: "/health" },
  { method: "get", path: "/v1/agents/responses" },
  { method: "post", path: "/v1/agents/responses" },
  { method: "get", path: "/v1/audio/transcriptions" },
  { method: "post", path: "/v1/audio/transcriptions" },
  { method: "get", path: "/v1/audio/transcriptions/{id}" },
  { method: "get", path: "/v1/backends" },
  { method: "get", path: "/v1/backends/status" },
  { method: "get", path: "/v1/chat/models" },
  { method: "post", path: "/v1/chat/completions" },
  { method: "post", path: "/v1/completions" },
  { method: "post", path: "/v1/ffmpeg/convert" },
  { method: "get", path: "/v1/images/generations" },
  { method: "post", path: "/v1/images/generations" },
  { method: "get", path: "/v1/images/generations/{id}" },
  { method: "get", path: "/v1/images/generations/{id}/artifacts/{index}" },
  { method: "get", path: "/v1/images/generations/{id}/reference" },
  { method: "get", path: "/v1/models" },
  { method: "post", path: "/v1/models" },
  { method: "get", path: "/v1/models/available" },
  { method: "post", path: "/v1/models/download" },
  { method: "post", path: "/v1/models/import-pack" },
  { method: "post", path: "/v1/models/load" },
  { method: "post", path: "/v1/models/switch" },
  { method: "post", path: "/v1/models/unload" },
  { method: "delete", path: "/v1/models/{id}" },
  { method: "get", path: "/v1/models/{id}" },
  { method: "put", path: "/v1/models/{id}" },
  { method: "get", path: "/v1/models/{id}/config-document" },
  { method: "put", path: "/v1/models/{id}/config-selection" },
  { method: "get", path: "/v1/plugins" },
  { method: "get", path: "/v1/plugins/events" },
  { method: "post", path: "/v1/plugins/import-pack" },
  { method: "post", path: "/v1/plugins/install" },
  { method: "get", path: "/v1/plugins/rpc" },
  { method: "delete", path: "/v1/plugins/{id}" },
  { method: "get", path: "/v1/plugins/{id}" },
  { method: "post", path: "/v1/plugins/{id}/disable" },
  { method: "post", path: "/v1/plugins/{id}/enable" },
  { method: "post", path: "/v1/plugins/{id}/start" },
  { method: "post", path: "/v1/plugins/{id}/stop" },
  { method: "delete", path: "/v1/sessions/{id}" },
  { method: "get", path: "/v1/sessions" },
  { method: "post", path: "/v1/sessions" },
  { method: "put", path: "/v1/sessions/{id}" },
  { method: "get", path: "/v1/sessions/{id}/messages" },
  { method: "get", path: "/v1/settings" },
  { method: "get", path: "/v1/settings/{pmid}" },
  { method: "put", path: "/v1/settings/{pmid}" },
  { method: "post", path: "/v1/setup/complete" },
  { method: "post", path: "/v1/setup/provision" },
  { method: "get", path: "/v1/setup/status" },
  { method: "post", path: "/v1/subtitles/render" },
  { method: "get", path: "/v1/system/diagnostics" },
  { method: "get", path: "/v1/system/gpu" },
  { method: "get", path: "/v1/tasks" },
  { method: "get", path: "/v1/tasks/{id}" },
  { method: "post", path: "/v1/tasks/{id}/cancel" },
  { method: "post", path: "/v1/tasks/{id}/restart" },
  { method: "get", path: "/v1/tasks/{id}/result" },
  { method: "delete", path: "/v1/ui-state/{key}" },
  { method: "get", path: "/v1/ui-state/{key}" },
  { method: "put", path: "/v1/ui-state/{key}" },
  { method: "get", path: "/v1/video/generations" },
  { method: "post", path: "/v1/video/generations" },
  { method: "get", path: "/v1/video/generations/{id}" },
  { method: "get", path: "/v1/video/generations/{id}/artifact" },
  { method: "get", path: "/v1/video/generations/{id}/reference" }
] as const satisfies readonly SmokeOperation[];

export const todoSmokeOperations = [] as const satisfies readonly SmokeOperation[];

export const futureCompatibilityScenarios = [
  "POST /v1/embeddings accepts single and batch inputs with deterministic usage fields",
  "POST /v1/rerank ranks documents and supports TEI-compatible response shape",
  "POST /v1/tokenize and POST /v1/detokenize round-trip text and token arrays",
  "POST /v1/responses supports OpenAI responses-compatible JSON and streaming output",
  "POST /v1/messages supports Anthropic messages-compatible JSON and streaming output",
  "POST /v1/messages/count_tokens counts Anthropic message tokens without generation",
  "POST /v1/infill covers prefix/suffix code infill prompts",
  "POST /v1/chat/templates/apply renders model chat templates without generation",
  "GET /v1/slots and POST /v1/slots/{id} cover slot save, restore, and erase",
  "POST /v1/lora-adapters loads global and per-request LoRA adapters",
  "POST /v1/chat/completions covers speculative decoding with draft model controls",
  "POST /v1/completions covers context-shift behavior for long prompts",
  "POST /v1/chat/completions covers required tool calls and tool result history",
  "POST /v1/chat/completions covers image content for vision-capable models",
  "POST /v1/completions covers image content for multimodal completion models",
  "POST /v1/embeddings covers image inputs for multimodal embedding models"
] as const;

export function operationKey(operation: SmokeOperation): string {
  return `${operation.method.toUpperCase()} ${operation.path}`;
}

export function documentedOperationKeys(openapi: OpenApiDocument): string[] {
  const keys: string[] = [];

  for (const [path, operations] of Object.entries(openapi.paths ?? {})) {
    for (const method of documentedMethods) {
      if (operations[method]) {
        keys.push(`${method.toUpperCase()} ${path}`);
      }
    }
  }

  return keys.toSorted();
}

export function jsonInit<T>(body: T, init: RequestInit = {}): RequestInit {
  return {
    ...init,
    body: JSON.stringify(body),
    headers: {
      ...jsonHeaders,
      ...init.headers
    }
  };
}

export async function expectJson<T>(
  server: SlabServerTestHarness,
  path: string,
  init?: RequestInit
): Promise<JsonResponse<T>> {
  const result = await server.requestJson<T>(path, init);
  expect(result.response.headers.get("content-type")).toContain("application/json");
  return result;
}

export async function expectError(
  server: SlabServerTestHarness,
  path: string,
  status: number,
  init?: RequestInit
): Promise<ServerErrorResponse> {
  const { response, body } = await expectJson<ServerErrorResponse>(server, path, init);

  expect(response.status).toBe(status);
  expect(body.message).toBeTypeOf("string");
  expect(body.code).toBeTypeOf("number");

  return body;
}

export async function expectOpenAiError(
  server: SlabServerTestHarness,
  path: string,
  status: number,
  init?: RequestInit
): Promise<OpenAiErrorResponse> {
  const { response, body } = await expectJson<OpenAiErrorResponse>(server, path, init);

  expect(response.status).toBe(status);
  expect(body.error?.message).toBeTypeOf("string");
  expect(body.error?.type).toBeTypeOf("string");

  return body;
}

export async function eventually<T>(
  label: string,
  assertion: () => Promise<T | false | null | undefined> | T | false | null | undefined,
  timeoutMs = 30_000,
  intervalMs = 250
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown;

  while (Date.now() < deadline) {
    try {
      const result = await assertion();
      if (result) {
        return result;
      }
    } catch (error) {
      lastError = error;
    }

    await new Promise((resolveDelay) => setTimeout(resolveDelay, intervalMs));
  }

  const suffix = lastError instanceof Error ? ` Last error: ${lastError.message}` : "";
  throw new Error(`${label} timed out after ${timeoutMs}ms.${suffix}`);
}

export async function waitForTask(
  server: SlabServerTestHarness,
  taskId: string,
  predicate: (task: TaskResponse) => boolean,
  timeoutMs = 30_000
): Promise<TaskResponse> {
  return eventually(
    `task ${taskId}`,
    async () => {
      const task = await expectJson<TaskResponse>(server, `/v1/tasks/${taskId}`);
      return predicate(task.body) ? task.body : null;
    },
    timeoutMs
  );
}

export async function buildCloudModelPackFile(modelId: string): Promise<File> {
  return buildZipFile(`${modelId}.slab`, {
    "manifest.json": JSON.stringify({
      family: "llama",
      id: modelId,
      label: `Smoke ${modelId}`,
      source: {
        kind: "cloud",
        provider_id: "smoke-provider",
        remote_model_id: modelId
      },
      status: "ready",
      version: 2
    })
  });
}

export async function buildPluginPackFile(pluginId: string): Promise<File> {
  const htmlPath = "ui/index.html";
  const html = "<!doctype html><html><body>Smoke plugin</body></html>";
  return buildZipFile(`${pluginId}.plugin.slab`, {
    [`${pluginId}/${htmlPath}`]: html,
    [`${pluginId}/plugin.json`]: JSON.stringify({
      id: pluginId,
      integrity: {
        filesSha256: {
          [htmlPath]: sha256Hex(html)
        }
      },
      manifestVersion: 1,
      name: "Smoke Plugin",
      permissions: {
        network: {
          allowHosts: [],
          mode: "blocked"
        }
      },
      runtime: {
        ui: {
          entry: htmlPath
        }
      },
      version: "0.1.0"
    })
  });
}

export function formDataWithFile(file: File): FormData {
  const body = new FormData();
  body.set("file", file);
  return body;
}

export async function expectWebSocketOpens(
  server: SlabServerTestHarness,
  path: string
): Promise<void> {
  const socket = await openWebSocket(server, path);
  socket.close();
}

export async function expectWebSocketJsonReply<T>(
  server: SlabServerTestHarness,
  path: string,
  payload: unknown
): Promise<T> {
  const socket = await openWebSocket(server, path);
  try {
    socket.send(typeof payload === "string" ? payload : JSON.stringify(payload));
    const text = await waitForWebSocketMessage(socket);
    return JSON.parse(text) as T;
  } finally {
    socket.close();
  }
}

async function buildZipFile(fileName: string, entries: Record<string, string>): Promise<File> {
  const bytes = buildStoredZip(entries);
  return new File([bytes], fileName, { type: "application/octet-stream" });
}

function sha256Hex(content: string): string {
  return createHash("sha256").update(content).digest("hex");
}

function buildStoredZip(entries: Record<string, string>): Uint8Array {
  const localRecords: Buffer[] = [];
  const centralRecords: Buffer[] = [];
  let offset = 0;

  for (const [path, content] of Object.entries(entries)) {
    const name = Buffer.from(path, "utf8");
    const data = Buffer.from(content, "utf8");
    const crc = crc32(data);
    const localHeader = Buffer.alloc(30);
    localHeader.writeUInt32LE(0x04034b50, 0);
    localHeader.writeUInt16LE(20, 4);
    localHeader.writeUInt16LE(0, 6);
    localHeader.writeUInt16LE(0, 8);
    localHeader.writeUInt16LE(0, 10);
    localHeader.writeUInt16LE(0, 12);
    localHeader.writeUInt32LE(crc, 14);
    localHeader.writeUInt32LE(data.byteLength, 18);
    localHeader.writeUInt32LE(data.byteLength, 22);
    localHeader.writeUInt16LE(name.byteLength, 26);
    localHeader.writeUInt16LE(0, 28);
    localRecords.push(localHeader, name, data);

    const centralHeader = Buffer.alloc(46);
    centralHeader.writeUInt32LE(0x02014b50, 0);
    centralHeader.writeUInt16LE(20, 4);
    centralHeader.writeUInt16LE(20, 6);
    centralHeader.writeUInt16LE(0, 8);
    centralHeader.writeUInt16LE(0, 10);
    centralHeader.writeUInt16LE(0, 12);
    centralHeader.writeUInt16LE(0, 14);
    centralHeader.writeUInt32LE(crc, 16);
    centralHeader.writeUInt32LE(data.byteLength, 20);
    centralHeader.writeUInt32LE(data.byteLength, 24);
    centralHeader.writeUInt16LE(name.byteLength, 28);
    centralHeader.writeUInt16LE(0, 30);
    centralHeader.writeUInt16LE(0, 32);
    centralHeader.writeUInt16LE(0, 34);
    centralHeader.writeUInt16LE(0, 36);
    centralHeader.writeUInt32LE(0, 38);
    centralHeader.writeUInt32LE(offset, 42);
    centralRecords.push(centralHeader, name);

    offset += localHeader.byteLength + name.byteLength + data.byteLength;
  }

  const localBytes = Buffer.concat(localRecords);
  const centralBytes = Buffer.concat(centralRecords);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(0, 4);
  end.writeUInt16LE(0, 6);
  end.writeUInt16LE(Object.keys(entries).length, 8);
  end.writeUInt16LE(Object.keys(entries).length, 10);
  end.writeUInt32LE(centralBytes.byteLength, 12);
  end.writeUInt32LE(localBytes.byteLength, 16);
  end.writeUInt16LE(0, 20);

  return Buffer.concat([localBytes, centralBytes, end]);
}

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc = CRC32_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

const CRC32_TABLE = new Uint32Array(
  Array.from({ length: 256 }, (_, index) => {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    return value >>> 0;
  })
);

function openWebSocket(server: SlabServerTestHarness, path: string): Promise<WebSocket> {
  return new Promise((resolveOpen, reject) => {
    const socket = new WebSocket(webSocketUrl(server.baseUrl, path));
    const timeout = setTimeout(() => {
      cleanup();
      socket.close();
      reject(new Error(`Timed out opening websocket ${path}`));
    }, 10_000);
    const cleanup = () => {
      clearTimeout(timeout);
      socket.removeEventListener("open", onOpen);
      socket.removeEventListener("error", onError);
    };
    const onOpen = () => {
      cleanup();
      resolveOpen(socket);
    };
    const onError = () => {
      cleanup();
      reject(new Error(`Failed to open websocket ${path}`));
    };
    socket.addEventListener("open", onOpen);
    socket.addEventListener("error", onError);
  });
}

function waitForWebSocketMessage(socket: WebSocket): Promise<string> {
  return new Promise((resolveMessage, reject) => {
    const timeout = setTimeout(() => {
      cleanup();
      reject(new Error("Timed out waiting for websocket message"));
    }, 10_000);
    const cleanup = () => {
      clearTimeout(timeout);
      socket.removeEventListener("message", onMessage);
      socket.removeEventListener("error", onError);
      socket.removeEventListener("close", onClose);
    };
    const onMessage = (event: MessageEvent) => {
      cleanup();
      resolveMessage(String(event.data));
    };
    const onError = () => {
      cleanup();
      reject(new Error("WebSocket failed while waiting for a message"));
    };
    const onClose = () => {
      cleanup();
      reject(new Error("WebSocket closed before a message arrived"));
    };
    socket.addEventListener("message", onMessage);
    socket.addEventListener("error", onError);
    socket.addEventListener("close", onClose);
  });
}

function webSocketUrl(baseUrl: string, path: string): string {
  const url = new URL(path, baseUrl);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}
