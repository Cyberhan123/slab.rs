import { describe, expect, it, vi } from "vitest";

import { postFormData } from "../index";

type MockFetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

describe("postFormData", () => {
  it("posts file uploads as multipart form data through the API client", async () => {
    const requests: Array<{ init?: RequestInit; input: RequestInfo | URL }> = [];
    const fetchMock = vi.fn<MockFetch>(async (input: RequestInfo | URL, init?: RequestInit) => {
      requests.push({ input, init });
      return new Response(JSON.stringify({ id: "model-1" }), {
        headers: { "content-type": "application/json" },
        status: 200,
      });
    });
    const result = await postFormData(
      "/v1/models/import-pack",
      new File(["pack"], "model.slab", { type: "application/octet-stream" }),
      { fetch: fetchMock },
    );

    expect(result).toEqual({ id: "model-1" });
    expect(fetchMock).toHaveBeenCalledOnce();
    expect(requests).toHaveLength(1);
    expect(requests[0].init?.method).toBe("POST");
    expect(String(requests[0].input)).toBe("http://127.0.0.1:3000/v1/models/import-pack");

    const body = requests[0].init?.body as FormData;
    const file = body.get("file");
    expect(file).toBeTruthy();
    expect((file as File).name).toBe("model.slab");
    expect(await (file as File).text()).toBe("pack");
  });

  it("maps standard API errors for multipart uploads", async () => {
    const fetchMock = vi.fn<MockFetch>(async () =>
      new Response(JSON.stringify({ code: 4000, data: null, message: "invalid pack" }), {
        headers: { "content-type": "application/json" },
        status: 400,
      }),
    );

    await expect(
      postFormData(
        "/v1/plugins/import-pack",
        new File(["pack"], "plugin.plugin.slab", { type: "application/octet-stream" }),
        { fetch: fetchMock },
      ),
    ).rejects.toMatchObject({
      code: 4000,
      message: "invalid pack",
      status: 400,
    });
  });
});
