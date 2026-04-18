import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  startSlabServerHarness,
  type SlabServerTestHarness
} from "../support/slab-server";

const externalBaseUrl = process.env.SLAB_SERVER_BASE_URL?.trim();

async function requestHealth(server: SlabServerTestHarness | undefined): Promise<Response> {
  if (!externalBaseUrl) {
    return server!.request("/health");
  }

  try {
    return await fetch(`${externalBaseUrl}/health`);
  } catch (error) {
    throw new Error(
      `Cannot reach slab-server at ${externalBaseUrl}. Start the server first or unset SLAB_SERVER_BASE_URL to use the local test harness.`,
      { cause: error }
    );
  }
}

describe("slab-server integration", () => {
  let server: SlabServerTestHarness | undefined;

  beforeAll(async () => {
    if (!externalBaseUrl) {
      server = await startSlabServerHarness();
    }
  });

  afterAll(async () => {
    await server?.stop();
  });

  it("responds to GET /health", async () => {
    const response = await requestHealth(server);

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
