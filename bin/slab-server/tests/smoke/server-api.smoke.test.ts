import { resolve } from "node:path";

import type { components, paths } from "@slab/api";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  startSlabServerHarness,
  type JsonResponse,
  type SlabServerTestHarness
} from "../support/slab-server";

type ApiPath = keyof paths;
type HttpMethod = "delete" | "get" | "post" | "put";
type OpenApiDocument = {
  openapi?: string;
  paths?: Record<string, Partial<Record<HttpMethod, unknown>>>;
};
type HealthResponse = {
  status?: string;
  version?: string;
};
type ServerErrorResponse = {
  code?: number;
  data?: unknown;
  message?: string;
};
type DeletedModelResponse = {
  id: string;
  status: string;
};
type Schema = components["schemas"];
type SmokeOperation = {
  method: HttpMethod;
  path: ApiPath;
};

const externalBaseUrl = process.env.SLAB_SERVER_BASE_URL?.trim();
const jsonHeaders = {
  "Content-Type": "application/json"
};
const documentedMethods: readonly HttpMethod[] = ["delete", "get", "post", "put"];

const executableSmokeOperations = [
  { method: "get", path: "/health" },
  { method: "post", path: "/v1/agents/spawn" },
  { method: "get", path: "/v1/agents/{id}/status" },
  { method: "post", path: "/v1/agents/{id}/shutdown" },
  { method: "post", path: "/v1/agents/{id}/approve" },
  { method: "post", path: "/v1/agents/{id}/interrupt" },
  { method: "get", path: "/v1/agents/{id}/events" },
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
  { method: "post", path: "/v1/plugins/import-pack" },
  { method: "post", path: "/v1/plugins/install" },
  { method: "delete", path: "/v1/plugins/{id}" },
  { method: "get", path: "/v1/plugins/{id}" },
  { method: "post", path: "/v1/plugins/{id}/disable" },
  { method: "post", path: "/v1/plugins/{id}/enable" },
  { method: "post", path: "/v1/plugins/{id}/start" },
  { method: "post", path: "/v1/plugins/{id}/stop" },
  { method: "delete", path: "/v1/sessions/{id}" },
  { method: "get", path: "/v1/sessions" },
  { method: "post", path: "/v1/sessions" },
  { method: "get", path: "/v1/sessions/{id}/messages" },
  { method: "get", path: "/v1/settings" },
  { method: "get", path: "/v1/settings/{pmid}" },
  { method: "put", path: "/v1/settings/{pmid}" },
  { method: "post", path: "/v1/setup/complete" },
  { method: "get", path: "/v1/setup/status" },
  { method: "post", path: "/v1/subtitles/render" },
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

const todoSmokeOperations = [
  { method: "post", path: "/v1/agents/{id}/input" },
  { method: "post", path: "/v1/setup/provision" },
  { method: "post", path: "/v1/tasks/{id}/restart" }
] as const satisfies readonly SmokeOperation[];

const futureCompatibilityScenarios = [
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

function operationKey(operation: SmokeOperation): string {
  return `${operation.method.toUpperCase()} ${operation.path}`;
}

function documentedOperationKeys(openapi: OpenApiDocument): string[] {
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

function jsonInit<T>(body: T, init: RequestInit = {}): RequestInit {
  return {
    ...init,
    body: JSON.stringify(body),
    headers: {
      ...jsonHeaders,
      ...init.headers
    }
  };
}

async function expectJson<T>(
  server: SlabServerTestHarness,
  path: string,
  init?: RequestInit
): Promise<JsonResponse<T>> {
  const result = await server.requestJson<T>(path, init);
  expect(result.response.headers.get("content-type")).toContain("application/json");
  return result;
}

async function expectError(
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

describe("slab-server smoke API", () => {
  let server: SlabServerTestHarness | undefined;

  beforeAll(async () => {
    server = await startSlabServerHarness({
      externalBaseUrl
    });
  });

  afterAll(async () => {
    await server?.stop();
  });

  it("serves health, OpenAPI docs, and a complete smoke coverage map", async () => {
    const health = await expectJson<HealthResponse>(server!, "/health");
    expect(health.response.ok).toBe(true);
    expect(health.body.status).toBe("ok");
    expect(typeof health.body.version).toBe("string");
    expect(health.body.version?.length ?? 0).toBeGreaterThan(0);

    const openapi = await expectJson<OpenApiDocument>(server!, "/api-docs/openapi.json");
    expect(openapi.response.ok).toBe(true);
    expect(openapi.body.openapi).toBeTypeOf("string");
    expect(openapi.body.paths).toBeTypeOf("object");
    expect(openapi.body.paths).toHaveProperty("/health");
    expect(openapi.body.paths).toHaveProperty("/v1/models");
    expect(openapi.body.paths).toHaveProperty("/v1/tasks/{id}/restart");

    const covered = [...executableSmokeOperations, ...todoSmokeOperations].map(operationKey).sort();
    expect(new Set(covered).size).toBe(covered.length);
    expect(documentedOperationKeys(openapi.body)).toEqual(covered);
  });

  it("returns CORS headers for browser preflight requests", async () => {
    const origin = "http://localhost:1420";
    const response = await server!.request("/v1/setup/status", {
      headers: {
        "Access-Control-Request-Headers": "Authorization",
        "Access-Control-Request-Method": "GET",
        Origin: origin
      },
      method: "OPTIONS"
    });

    expect(response.status).toBeLessThan(300);
    expect(response.headers.get("access-control-allow-origin")).toBe(origin);
    expect(response.headers.get("access-control-allow-methods")).toBeTruthy();
    expect(response.headers.get("access-control-allow-headers")).toBeTruthy();
  });

  it("covers setup, settings, backend, and system endpoints without provisioning", async () => {
    const initial = await expectJson<Schema["SetupStatusResponse"]>(server!, "/v1/setup/status");
    expect(initial.response.ok).toBe(true);
    expect(initial.body.ffmpeg.name).toBe("ffmpeg");
    expect(Array.isArray(initial.body.backends)).toBe(true);

    const completeRequest: Schema["CompleteSetupRequest"] = { initialized: true };
    const completed = await expectJson<Schema["SetupStatusResponse"]>(
      server!,
      "/v1/setup/complete",
      jsonInit(completeRequest, { method: "POST" })
    );
    expect(completed.response.ok).toBe(true);
    expect(completed.body.initialized).toBe(true);

    const restored = await expectJson<Schema["SetupStatusResponse"]>(
      server!,
      "/v1/setup/complete",
      jsonInit({ initialized: initial.body.initialized } satisfies Schema["CompleteSetupRequest"], {
        method: "POST"
      })
    );
    expect(restored.response.ok).toBe(true);
    expect(restored.body.initialized).toBe(initial.body.initialized);

    const settings = await expectJson<Schema["SettingsDocumentView"]>(server!, "/v1/settings");
    expect(settings.response.ok).toBe(true);
    expect(Array.isArray(settings.body.sections)).toBe(true);

    await expectError(server!, "/v1/settings/smoke.missing", 404);
    await expectError(
      server!,
      "/v1/settings/smoke.missing",
      404,
      jsonInit({ op: "unset" } satisfies Schema["UpdateSettingCommand"], { method: "PUT" })
    );

    const backends = await expectJson<Schema["BackendListResponse"]>(server!, "/v1/backends");
    expect(backends.response.ok).toBe(true);
    expect(Array.isArray(backends.body.backends)).toBe(true);

    const backendStatus = await expectJson<Schema["BackendStatusResponse"]>(
      server!,
      "/v1/backends/status?backend_id=ggml.llama"
    );
    expect(backendStatus.response.ok).toBe(true);
    expect(backendStatus.body.backend).toBe("ggml.llama");
    expect(backendStatus.body.status).toBeTypeOf("string");

    const gpu = await expectJson<Schema["GpuStatusResponse"]>(server!, "/v1/system/gpu");
    expect(gpu.response.ok).toBe(true);
    expect(gpu.body.available).toBeTypeOf("boolean");
    expect(gpu.body.backend).toBeTypeOf("string");
    expect(Array.isArray(gpu.body.devices)).toBe(true);
  });

  it("covers model catalog and chat compatibility routes without inference", async () => {
    const createRequest: Schema["CreateModelRequest"] = {
      backend_id: "ggml.llama",
      capabilities: ["chat_generation", "text_generation"],
      display_name: "Vitest Smoke Local Chat Model",
      kind: "local"
    };
    const created = await expectJson<Schema["UnifiedModelResponse"]>(
      server!,
      "/v1/models",
      jsonInit(createRequest, { method: "POST" })
    );
    expect(created.response.ok).toBe(true);
    expect(created.body.display_name).toBe(createRequest.display_name);
    expect(created.body.kind).toBe("local");
    expect(created.body.backend_id).toBe("ggml.llama");
    expect(created.body.capabilities).toContain("chat_generation");

    try {
      const listed = await expectJson<Schema["UnifiedModelResponse"][]>(server!, "/v1/models");
      expect(listed.response.ok).toBe(true);
      expect(listed.body.some((model) => model.id === created.body.id)).toBe(true);

      const filtered = await expectJson<Schema["UnifiedModelResponse"][]>(
        server!,
        "/v1/models?capability=chat_generation"
      );
      expect(filtered.body.some((model) => model.id === created.body.id)).toBe(true);

      const chatModels = await expectJson<Schema["ChatModelOption"][]>(server!, "/v1/chat/models");
      const chatModel = chatModels.body.find((model) => model.id === created.body.id);
      expect(chatModel).toBeDefined();
      expect(chatModel?.source).toBe("local");
      expect(chatModel?.downloaded).toBe(false);

      const fetched = await expectJson<Schema["UnifiedModelResponse"]>(
        server!,
        `/v1/models/${created.body.id}`
      );
      expect(fetched.body.id).toBe(created.body.id);

      const updated = await expectJson<Schema["UnifiedModelResponse"]>(
        server!,
        `/v1/models/${created.body.id}`,
        jsonInit({ display_name: "Vitest Smoke Renamed Model" } satisfies Schema["UpdateModelRequest"], {
          method: "PUT"
        })
      );
      expect(updated.body.display_name).toBe("Vitest Smoke Renamed Model");

      await expectError(server!, `/v1/models/${created.body.id}/config-document`, 400);
      await expectError(
        server!,
        `/v1/models/${created.body.id}/config-selection`,
        400,
        jsonInit({} satisfies Schema["UpdateModelConfigSelectionRequest"], { method: "PUT" })
      );
    } finally {
      const deleted = await expectJson<DeletedModelResponse>(
        server!,
        `/v1/models/${created.body.id}`,
        { method: "DELETE" }
      );
      expect(deleted.response.ok).toBe(true);
      expect(deleted.body).toEqual({
        id: created.body.id,
        status: "deleted"
      });
    }

    await expectError(
      server!,
      "/v1/chat/completions",
      400,
      jsonInit(
        {
          messages: [{ content: "hello", role: "user" }],
          model: "missing-model",
          n: 2,
          stream: true
        } satisfies Schema["ChatCompletionRequest"],
        { method: "POST" }
      )
    );
    await expectError(
      server!,
      "/v1/completions",
      400,
      jsonInit(
        {
          model: "missing-model",
          n: 2,
          prompt: "hello",
          stream: true
        } satisfies Schema["CompletionRequest"],
        { method: "POST" }
      )
    );
    await expectError(server!, "/v1/models/available?repo_id=", 400);
    await expectError(
      server!,
      "/v1/models/load",
      400,
      jsonInit({} satisfies Schema["LoadModelRequest"], { method: "POST" })
    );
    await expectError(
      server!,
      "/v1/models/switch",
      400,
      jsonInit({} satisfies Schema["SwitchModelRequest"], { method: "POST" })
    );
    await expectError(
      server!,
      "/v1/models/unload",
      400,
      jsonInit({} satisfies Schema["UnloadModelRequest"], { method: "POST" })
    );
    await expectError(
      server!,
      "/v1/models/download",
      404,
      jsonInit({ model_id: "missing-model" } satisfies Schema["DownloadModelRequest"], {
        method: "POST"
      })
    );

    const emptyPack = new FormData();
    const importPack = await server!.requestFormData("/v1/models/import-pack", emptyPack, {
      method: "POST"
    });
    expect(importPack.status).toBe(400);
  });

  it("covers sessions and UI state persistence routes", async () => {
    const created = await expectJson<Schema["SessionResponse"]>(
      server!,
      "/v1/sessions",
      jsonInit({ name: "Vitest smoke session" } satisfies Schema["CreateSessionRequest"], {
        method: "POST"
      })
    );
    expect(created.response.ok).toBe(true);
    expect(created.body.name).toBe("Vitest smoke session");

    const listed = await expectJson<Schema["SessionResponse"][]>(server!, "/v1/sessions");
    expect(listed.body.some((session) => session.id === created.body.id)).toBe(true);

    const messages = await expectJson<Schema["MessageResponse"][]>(
      server!,
      `/v1/sessions/${created.body.id}/messages`
    );
    expect(messages.response.ok).toBe(true);
    expect(Array.isArray(messages.body)).toBe(true);

    const deleted = await expectJson<unknown>(server!, `/v1/sessions/${created.body.id}`, {
      method: "DELETE"
    });
    expect(deleted.response.ok).toBe(true);

    const key = `smoke.${Date.now()}`;
    const updatedState = await expectJson<Schema["UiStateValueResponse"]>(
      server!,
      `/v1/ui-state/${encodeURIComponent(key)}`,
      jsonInit({ value: "ready" } satisfies Schema["UpdateUiStateRequest"], { method: "PUT" })
    );
    expect(updatedState.body.key).toBe(key);
    expect(updatedState.body.value).toBe("ready");

    const fetchedState = await expectJson<Schema["UiStateValueResponse"]>(
      server!,
      `/v1/ui-state/${encodeURIComponent(key)}`
    );
    expect(fetchedState.body.value).toBe("ready");

    const deletedState = await expectJson<Schema["UiStateDeleteResponse"]>(
      server!,
      `/v1/ui-state/${encodeURIComponent(key)}`,
      { method: "DELETE" }
    );
    expect(deletedState.body.deleted).toBe(true);
  });

  it("covers plugin management routes with validation and not-found paths", async () => {
    const plugins = await expectJson<Schema["PluginResponse"][]>(server!, "/v1/plugins");
    expect(plugins.response.ok).toBe(true);
    expect(Array.isArray(plugins.body)).toBe(true);

    await expectError(server!, "/v1/plugins/missing-plugin", 404);
    await expectError(
      server!,
      "/v1/plugins/install",
      400,
      jsonInit(
        {
          pluginId: ""
        } satisfies Schema["InstallPluginRequest"],
        { method: "POST" }
      )
    );

    const emptyPack = new FormData();
    const importPack = await server!.requestFormData("/v1/plugins/import-pack", emptyPack, {
      method: "POST"
    });
    expect(importPack.status).toBe(400);

    for (const path of [
      "/v1/plugins/missing-plugin/enable",
      "/v1/plugins/missing-plugin/disable",
      "/v1/plugins/missing-plugin/start"
    ]) {
      await expectError(server!, path, 404, { method: "POST" });
    }

    await expectError(
      server!,
      "/v1/plugins/missing-plugin/stop",
      404,
      jsonInit({ lastError: "smoke" } satisfies Schema["StopPluginRequest"], { method: "POST" })
    );
    await expectError(server!, "/v1/plugins/missing-plugin", 404, { method: "DELETE" });
  });

  it("covers tasks and media routes without running runtime work", async () => {
    const tasks = await expectJson<Schema["TaskResponse"][]>(server!, "/v1/tasks");
    expect(tasks.response.ok).toBe(true);
    expect(Array.isArray(tasks.body)).toBe(true);

    await expectError(server!, "/v1/tasks/missing-task", 404);
    await expectError(server!, "/v1/tasks/missing-task/result", 404);
    await expectError(server!, "/v1/tasks/missing-task/cancel", 404, { method: "POST" });

    const audioTasks = await expectJson<Schema["AudioTranscriptionTaskResponse"][]>(
      server!,
      "/v1/audio/transcriptions"
    );
    expect(audioTasks.response.ok).toBe(true);
    expect(Array.isArray(audioTasks.body)).toBe(true);
    await expectError(server!, "/v1/audio/transcriptions/missing-task", 404);
    await expectError(
      server!,
      "/v1/audio/transcriptions",
      400,
      jsonInit({ path: "relative.wav" } satisfies Schema["AudioTranscriptionRequest"], {
        method: "POST"
      })
    );

    const imageTasks = await expectJson<Schema["ImageGenerationTaskResponse"][]>(
      server!,
      "/v1/images/generations"
    );
    expect(imageTasks.response.ok).toBe(true);
    expect(Array.isArray(imageTasks.body)).toBe(true);
    await expectError(server!, "/v1/images/generations/missing-task", 404);
    await expectError(server!, "/v1/images/generations/missing-task/artifacts/0", 404);
    await expectError(server!, "/v1/images/generations/missing-task/reference", 404);
    await expectError(
      server!,
      "/v1/images/generations",
      400,
      jsonInit(
        {
          mode: "img2img",
          model: "missing-model",
          prompt: "smoke"
        } satisfies Schema["ImageGenerationRequest"],
        { method: "POST" }
      )
    );

    const videoTasks = await expectJson<Schema["VideoGenerationTaskResponse"][]>(
      server!,
      "/v1/video/generations"
    );
    expect(videoTasks.response.ok).toBe(true);
    expect(Array.isArray(videoTasks.body)).toBe(true);
    await expectError(server!, "/v1/video/generations/missing-task", 404);
    await expectError(server!, "/v1/video/generations/missing-task/artifact", 404);
    await expectError(server!, "/v1/video/generations/missing-task/reference", 404);
    await expectError(
      server!,
      "/v1/video/generations",
      400,
      jsonInit(
        {
          model: "",
          prompt: "smoke"
        } satisfies Schema["VideoGenerationRequest"],
        { method: "POST" }
      )
    );

    await expectError(
      server!,
      "/v1/ffmpeg/convert",
      400,
      jsonInit(
        {
          output_format: "mp3",
          source_path: resolve("missing-smoke-input.wav")
        } satisfies Schema["ConvertRequest"],
        { method: "POST" }
      )
    );
    await expectError(
      server!,
      "/v1/subtitles/render",
      400,
      jsonInit(
        {
          entries: [{ end_ms: 1000, start_ms: 0, text: "hello" }],
          format: "srt",
          source_path: "relative.mp4",
          variant: "source"
        } satisfies Schema["RenderSubtitleRequest"],
        { method: "POST" }
      )
    );
  });

  it("covers agent control and workspace LSP routes without running agent work", async () => {
    await expectError(
      server!,
      "/v1/agents/spawn",
      400,
      jsonInit(
        {
          session_id: "",
          messages: []
        } satisfies Schema["SpawnAgentRequest"],
        { method: "POST" }
      )
    );
    await expectError(server!, "/v1/agents/missing-agent/status", 404);
    await expectError(server!, "/v1/agents/missing-agent/shutdown", 404, { method: "POST" });
    await expectError(server!, "/v1/agents/missing-agent/interrupt", 404, { method: "POST" });

    const approval = await expectJson<Schema["AgentApproveResponse"]>(
      server!,
      "/v1/agents/missing-agent/approve",
      jsonInit(
        {
          approved: true,
          call_id: "missing-call"
        } satisfies Schema["AgentApproveRequest"],
        { method: "POST" }
      )
    );
    expect(approval.response.ok).toBe(true);
    expect(approval.body.delivered).toBe(false);

    const events = await server!.request("/v1/agents/missing-agent/events");
    expect(events.ok).toBe(true);
    expect(events.headers.get("content-type")).toContain("text/event-stream");
    await events.body?.cancel();

    const lspUpgradeMissing = await server!.request("/v1/workspace/lsp/typescript");
    expect(lspUpgradeMissing.status).not.toBe(404);
  });
});

describe("slab-server admin authentication smoke", () => {
  let server: SlabServerTestHarness | undefined;

  beforeAll(async () => {
    if (externalBaseUrl) {
      return;
    }

    server = await startSlabServerHarness({
      adminToken: "vitest-admin-token"
    });
  });

  afterAll(async () => {
    await server?.stop();
  });

  it.skipIf(Boolean(externalBaseUrl))("requires bearer auth for management routes", async () => {
    const settings = await server!.request("/v1/settings");
    expect(settings.status).toBe(401);

    const backends = await server!.request("/v1/backends");
    expect(backends.status).toBe(401);

    const authorized = await expectJson<Schema["SettingsDocumentView"]>(server!, "/v1/settings", {
      headers: {
        Authorization: "Bearer vitest-admin-token"
      }
    });
    expect(authorized.response.ok).toBe(true);
    expect(Array.isArray(authorized.body.sections)).toBe(true);
  });
});

describe("slab-server current smoke TODOs", () => {
  for (const operation of todoSmokeOperations) {
    it.todo(`${operation.method.toUpperCase()} ${operation.path} has an executable smoke test`);
  }
});

describe("slab-server future compatibility smoke TODOs", () => {
  for (const scenario of futureCompatibilityScenarios) {
    it.todo(scenario);
  }
});
