import { describe, expect, it } from "vitest";

const baseUrl = process.env.SLAB_SERVER_BASE_URL ?? "http://127.0.0.1:3000";

describe("slab-server integration", () => {
  it("responds to GET /health", async () => {
    let response: Response;
    try {
      response = await fetch(`${baseUrl}/health`);
    } catch (error) {
      throw new Error(
        `Cannot reach slab-server at ${baseUrl}. Start the server first or set SLAB_SERVER_BASE_URL to a reachable endpoint.`,
        { cause: error }
      );
    }

    expect(response.ok).toBe(true);

    const payload = (await response.json()) as {
      status?: string;
      version?: string;
    };

    expect(payload.status).toBe("ok");
    expect(typeof payload.version).toBe("string");
    expect(payload.version?.length ?? 0).toBeGreaterThan(0);
  });
});
