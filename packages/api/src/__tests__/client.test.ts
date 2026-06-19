import { describe, expect, it, vi } from "vitest";

import { createSlabApiFetchClient } from "../index";

type MockFetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

describe("createSlabApiFetchClient", () => {
  it("injects the current admin token into API requests", async () => {
    const requests: Request[] = [];
    const fetchMock = vi.fn<MockFetch>(async (input, init) => {
      const request = input instanceof Request ? input : new Request(input, init);
      requests.push(request);
      return new Response(
        JSON.stringify({
          schema_version: 2,
          sections: [],
          settings_path: "settings.json",
          warnings: [],
        }),
        {
          headers: { "content-type": "application/json" },
          status: 200,
        },
      );
    });
    const client = createSlabApiFetchClient({
      fetch: fetchMock,
      getAdminToken: () => " admin-token ",
    });

    await client.GET("/v1/settings");

    expect(fetchMock).toHaveBeenCalledOnce();
    expect(requests).toHaveLength(1);
    expect(requests[0].headers.get("authorization")).toBe("Bearer admin-token");
  });
});
