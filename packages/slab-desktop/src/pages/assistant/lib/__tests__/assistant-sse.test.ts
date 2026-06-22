import { afterEach, describe, expect, it, vi } from 'vitest';

import { nextReconnectDelayMs, readAssistantSseStream } from '../assistant-sse';

function responseFromText(text: string) {
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(text));
      controller.close();
    },
  });

  return new Response(stream, {
    status: 200,
    statusText: 'OK',
  });
}

describe('assistant SSE fetch stream', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('sends Last-Event-ID and parses SSE frames', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>().mockResolvedValue(
      responseFromText(
        [
          'id: 8',
          'data: {"type":"response.output_text.delta","delta":"hello"}',
          '',
          ': keepalive',
          '',
          'id: 9',
          'data: first',
          'data: second',
          '',
        ].join('\n'),
      ),
    );
    vi.stubGlobal('fetch', fetchMock);
    const messages: Array<{ data: string; id: string | null }> = [];

    await readAssistantSseStream('http://localhost/events', {
      lastEventId: 7,
      onMessage: (message) => messages.push(message),
    });

    const firstRequestInit = fetchMock.mock.calls[0]?.[1];
    expect(firstRequestInit).toBeDefined();
    expect(new Headers(firstRequestInit?.headers).get('Last-Event-ID')).toBe('7');
    expect(messages).toEqual([
      {
        data: '{"type":"response.output_text.delta","delta":"hello"}',
        id: '8',
      },
      {
        data: 'first\nsecond',
        id: '9',
      },
    ]);
  });

  it('does not attach Last-Event-ID when no prior event exists', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(responseFromText('data: ok\n\n'));
    vi.stubGlobal('fetch', fetchMock);

    await readAssistantSseStream('http://localhost/events', {
      onMessage: () => {},
    });

    const firstRequestInit = fetchMock.mock.calls[0]?.[1];
    expect(firstRequestInit).toBeDefined();
    expect(new Headers(firstRequestInit?.headers).has('Last-Event-ID')).toBe(false);
  });

  it('does not attach Last-Event-ID when resume is disabled', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(responseFromText('data: ok\n\n'));
    vi.stubGlobal('fetch', fetchMock);

    await readAssistantSseStream('http://localhost/events', {
      lastEventId: null,
      onMessage: () => {},
    });

    const firstRequestInit = fetchMock.mock.calls[0]?.[1];
    expect(firstRequestInit).toBeDefined();
    expect(new Headers(firstRequestInit?.headers).has('Last-Event-ID')).toBe(false);
  });

  it('caps reconnect delay at thirty seconds plus jitter', () => {
    const delay = nextReconnectDelayMs(10);
    expect(delay).toBeGreaterThanOrEqual(30_000);
    expect(delay).toBeLessThanOrEqual(30_300);
  });
});
