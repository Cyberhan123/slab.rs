import { describe, expect, it } from "vitest";

const baseUrl = process.env.SLAB_SERVER_BASE_URL ?? "http://127.0.0.1:3000";
const adminToken = process.env.SLAB_ADMIN_TOKEN;

async function fetchOrExplain(path: string, init?: RequestInit): Promise<Response> {
  try {
    return await fetch(`${baseUrl}${path}`, init);
  } catch (error) {
    throw new Error(
      `Cannot reach slab-server at ${baseUrl}. Start the server first or set SLAB_SERVER_BASE_URL to a reachable endpoint.`,
      { cause: error }
    );
  }
}

describe("slab-server unit migration smoke tests", () => {
  it("responds to GET /health", async () => {
    const response = await fetchOrExplain("/health");
    expect(response.ok).toBe(true);

    const payload = (await response.json()) as {
      status?: string;
      version?: string;
    };

    expect(payload.status).toBe("ok");
    expect(typeof payload.version).toBe("string");
    expect(payload.version?.length ?? 0).toBeGreaterThan(0);
  });

  it("serves OpenAPI docs with core paths", async () => {
    const response = await fetchOrExplain("/api-docs/openapi.json");
    expect(response.ok).toBe(true);

    const payload = (await response.json()) as {
      openapi?: string;
      paths?: Record<string, unknown>;
    };

    expect(typeof payload.openapi).toBe("string");
    expect(payload.paths).toBeTypeOf("object");
    expect(payload.paths).toHaveProperty("/health");
    expect(payload.paths).toHaveProperty("/v1/models");
  });

  it("returns CORS headers for preflight requests", async () => {
    const response = await fetchOrExplain("/v1/setup/status", {
      method: "OPTIONS",
      headers: {
        Origin: "https://example.com",
        "Access-Control-Request-Method": "GET",
        "Access-Control-Request-Headers": "Authorization"
      }
    });

    expect(response.status).toBeLessThan(300);
    expect(response.headers.get("access-control-allow-origin")).toBeTruthy();
    expect(response.headers.get("access-control-allow-methods")).toBeTruthy();
    expect(response.headers.get("access-control-allow-headers")).toBeTruthy();
  });

  it.skipIf(!adminToken)("authorizes admin routes with SLAB_ADMIN_TOKEN", async () => {
    const unauthorized = await fetchOrExplain("/v1/settings");
    expect(unauthorized.status).toBe(401);

    const authorized = await fetchOrExplain("/v1/settings", {
      headers: {
        Authorization: `Bearer ${adminToken}`
      }
    });
    expect(authorized.ok).toBe(true);
  });
});