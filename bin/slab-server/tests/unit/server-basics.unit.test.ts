import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  startSlabServerHarness,
  type JsonResponse,
  type SlabServerTestHarness
} from "../support/slab-server";

interface HealthResponse {
  status?: string;
  version?: string;
}

interface OpenApiResponse {
  openapi?: string;
  paths?: Record<string, unknown>;
}

interface SetupStatusResponse {
  initialized?: boolean;
  ffmpeg?: {
    name?: string;
    installed?: boolean;
    version?: string | null;
  };
  backends?: Array<{
    name?: string;
    installed?: boolean;
    version?: string | null;
  }>;
}

interface ModelResponse {
  id: string;
  display_name: string;
  kind: string;
  backend_id?: string | null;
  capabilities: string[];
  status: string;
}

interface ChatModelResponse {
  id: string;
  display_name: string;
  source: string;
  downloaded: boolean;
  pending: boolean;
  backend_id?: string | null;
}

interface DeletedModelResponse {
  id: string;
  status: string;
}

interface SettingsDocumentView {
  sections?: unknown[];
}

async function expectJson<T>(
  harness: SlabServerTestHarness,
  path: string,
  init?: RequestInit
): Promise<JsonResponse<T>> {
  const { response, body } = await harness.requestJson<T>(path, init);
  return { response, body };
}

describe("slab-server unit migration tests", () => {
  let server: SlabServerTestHarness | undefined;

  beforeAll(async () => {
    server = await startSlabServerHarness();
  });

  afterAll(async () => {
    await server?.stop();
  });

  it("responds to GET /health", async () => {
    const { response, body } = await expectJson<HealthResponse>(server!, "/health");

    expect(response.ok).toBe(true);
    expect(body.status).toBe("ok");
    expect(typeof body.version).toBe("string");
    expect(body.version?.length ?? 0).toBeGreaterThan(0);
  });

  it("serves OpenAPI docs with core paths", async () => {
    const { response, body } = await expectJson<OpenApiResponse>(server!, "/api-docs/openapi.json");

    expect(response.ok).toBe(true);
    expect(typeof body.openapi).toBe("string");
    expect(body.paths).toBeTypeOf("object");
    expect(body.paths).toHaveProperty("/health");
    expect(body.paths).toHaveProperty("/v1/models");
    expect(body.paths).toHaveProperty("/v1/settings");
    expect(body.paths).toHaveProperty("/v1/setup/status");
  });

  it("returns CORS headers for preflight requests", async () => {
    const origin = "http://localhost:1420";
    const response = await server!.request("/v1/setup/status", {
      method: "OPTIONS",
      headers: {
        Origin: origin,
        "Access-Control-Request-Method": "GET",
        "Access-Control-Request-Headers": "Authorization"
      }
    });

    expect(response.status).toBeLessThan(300);
    expect(response.headers.get("access-control-allow-origin")).toBe(origin);
    expect(response.headers.get("access-control-allow-methods")).toBeTruthy();
    expect(response.headers.get("access-control-allow-headers")).toBeTruthy();
  });

  it("round-trips setup initialized state through /v1/setup/complete", async () => {
    const initial = await expectJson<SetupStatusResponse>(server!, "/v1/setup/status");
    expect(initial.response.ok).toBe(true);
    expect(initial.body.ffmpeg?.name).toBe("ffmpeg");
    expect(Array.isArray(initial.body.backends)).toBe(true);

    const enabled = await expectJson<SetupStatusResponse>(server!, "/v1/setup/complete", {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify({ initialized: true })
    });
    expect(enabled.response.ok).toBe(true);
    expect(enabled.body.initialized).toBe(true);

    const reset = await expectJson<SetupStatusResponse>(server!, "/v1/setup/complete", {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify({ initialized: false })
    });
    expect(reset.response.ok).toBe(true);
    expect(reset.body.initialized).toBe(false);
  });

  it("creates, lists, exposes, and deletes local chat models", async () => {
    const created = await expectJson<ModelResponse>(server!, "/v1/models", {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify({
        display_name: "Vitest Local Chat Model",
        kind: "local",
        backend_id: "ggml.llama"
      })
    });

    expect(created.response.ok).toBe(true);
    expect(created.body.display_name).toBe("Vitest Local Chat Model");
    expect(created.body.kind).toBe("local");
    expect(created.body.backend_id).toBe("ggml.llama");
    expect(created.body.capabilities).toContain("chat_generation");
    expect(created.body.capabilities).toContain("text_generation");
    expect(created.body.status).toBe("not_downloaded");

    const listed = await expectJson<ModelResponse[]>(server!, "/v1/models");
    expect(listed.response.ok).toBe(true);
    expect(listed.body.some((model) => model.id === created.body.id)).toBe(true);

    const chatModels = await expectJson<ChatModelResponse[]>(server!, "/v1/chat/models");
    expect(chatModels.response.ok).toBe(true);

    const chatModel = chatModels.body.find((model) => model.id === created.body.id);
    expect(chatModel).toBeDefined();
    expect(chatModel?.source).toBe("local");
    expect(chatModel?.downloaded).toBe(false);
    expect(chatModel?.pending).toBe(false);
    expect(chatModel?.backend_id).toBe("ggml.llama");

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

    const listedAfterDelete = await expectJson<ModelResponse[]>(server!, "/v1/models");
    expect(listedAfterDelete.body.some((model) => model.id === created.body.id)).toBe(false);
  });

  it("keeps management routes open when no admin token is configured", async () => {
    const { response, body } = await expectJson<SettingsDocumentView>(server!, "/v1/settings");

    expect(response.ok).toBe(true);
    expect(Array.isArray(body.sections)).toBe(true);
  });
});

describe("slab-server admin authentication migration tests", () => {
  let server: SlabServerTestHarness | undefined;

  beforeAll(async () => {
    server = await startSlabServerHarness({
      adminToken: "vitest-admin-token"
    });
  });

  afterAll(async () => {
    await server?.stop();
  });

  it("authorizes /v1/settings with SLAB_ADMIN_TOKEN", async () => {
    const unauthorized = await server!.request("/v1/settings");
    expect(unauthorized.status).toBe(401);

    const { response, body } = await expectJson<SettingsDocumentView>(server!, "/v1/settings", {
      headers: {
        Authorization: "Bearer vitest-admin-token"
      }
    });

    expect(response.ok).toBe(true);
    expect(Array.isArray(body.sections)).toBe(true);
  });
});
