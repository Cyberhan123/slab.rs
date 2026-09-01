import { describe, expect, it, vi } from "vitest";

import { waitForApiServer } from "../health";

type MockFetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

const okResponse = () => new Response('{"ok":true}', { status: 200 });

describe("waitForApiServer", () => {
  it("resolves immediately when the server is already healthy", async () => {
    const fetchMock = vi.fn<MockFetch>(async () => okResponse());

    await expect(
      waitForApiServer({ fetch: fetchMock, intervalMs: 1, timeoutMs: 100 }),
    ).resolves.toBe(true);

    expect(fetchMock).toHaveBeenCalledOnce();
    expect(String(fetchMock.mock.calls[0][0])).toBe("http://127.0.0.1:3000/health");
  });

  it("retries while the connection is refused and resolves once healthy", async () => {
    let attempts = 0;
    const fetchMock = vi.fn<MockFetch>(async () => {
      attempts += 1;
      if (attempts < 3) {
        throw new TypeError("Failed to fetch");
      }
      return okResponse();
    });

    await expect(
      waitForApiServer({ fetch: fetchMock, intervalMs: 1, timeoutMs: 1_000 }),
    ).resolves.toBe(true);

    expect(attempts).toBe(3);
  });

  it("keeps polling on non-2xx responses and gives up after the timeout", async () => {
    const fetchMock = vi.fn<MockFetch>(async () => new Response("unavailable", { status: 503 }));

    await expect(
      waitForApiServer({ fetch: fetchMock, intervalMs: 1, timeoutMs: 10 }),
    ).resolves.toBe(false);

    expect(fetchMock.mock.calls.length).toBeGreaterThan(1);
  });

  it("resolves false without probing when the signal is already aborted", async () => {
    const fetchMock = vi.fn<MockFetch>(async () => okResponse());
    const controller = new AbortController();
    controller.abort();

    await expect(
      waitForApiServer({
        fetch: fetchMock,
        signal: controller.signal,
        intervalMs: 1,
        timeoutMs: 100,
      }),
    ).resolves.toBe(false);

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("honors a custom base url and path", async () => {
    const fetchMock = vi.fn<MockFetch>(async () => okResponse());

    await expect(
      waitForApiServer({
        baseUrl: "http://10.0.0.2:4000/",
        path: "healthz",
        fetch: fetchMock,
        intervalMs: 1,
        timeoutMs: 100,
      }),
    ).resolves.toBe(true);

    expect(String(fetchMock.mock.calls[0][0])).toBe("http://10.0.0.2:4000/healthz");
  });
});
