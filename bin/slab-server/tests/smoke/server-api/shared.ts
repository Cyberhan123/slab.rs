import type { components, paths } from "@slab/api";
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
  { method: "get", path: "/v1/setup/status" },
  { method: "post", path: "/v1/subtitles/render" },
  { method: "get", path: "/v1/system/diagnostics" },
  { method: "get", path: "/v1/system/gpu" },
  { method: "get", path: "/v1/tasks" },
  { method: "get", path: "/v1/tasks/{id}" },
  { method: "post", path: "/v1/tasks/{id}/cancel" },
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

export const todoSmokeOperations = [
  { method: "post", path: "/v1/setup/provision" },
  { method: "post", path: "/v1/tasks/{id}/restart" }
] as const satisfies readonly SmokeOperation[];

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

  return keys.sort();
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
