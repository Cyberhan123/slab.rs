

export interface CreateWebSocketFetchOptions {
  /**
   * WebSocket endpoint URL.
   * @default 'ws://localhost:3000/v1/agents/responses'
   */
  url?: string;
}

export function createWebSocketFetch(
  options?: CreateWebSocketFetchOptions,
) {
  const wsUrl = options?.url ?? 'ws://localhost:3000/v1/agents/responses';

  let ws: WebSocket | null = null;
  let connecting: Promise<WebSocket> | null = null;
  let busy = false;


  function getConnection(authorization?: string): Promise<WebSocket> {
    if (ws?.readyState === WebSocket.OPEN && !busy) {
      return Promise.resolve(ws);
    }

    if (connecting && !busy) return connecting;

    connecting = new Promise<WebSocket>((resolve, reject) => {
      const finalUrl = authorization 
        ? `${wsUrl}${wsUrl.includes('?') ? '&' : '?'}token=${encodeURIComponent(authorization)}`
        : wsUrl;

      const socket = new WebSocket(finalUrl);


      const onOpen = () => {
        ws = socket;
        connecting = null;
        cleanup();
        resolve(socket);
      };

      const onError = (err: Event) => {
        if (connecting) {
          connecting = null;
          cleanup();
          reject(err);
        }
      };

      const onClose = () => {
        if (ws === socket) ws = null;
        cleanup();
      };

      const cleanup = () => {
        socket.removeEventListener('open', onOpen);
        socket.removeEventListener('error', onError);
        socket.removeEventListener('close', onClose);
      };

      socket.addEventListener('open', onOpen);
      socket.addEventListener('error', onError);
      socket.addEventListener('close', onClose);
    });

    return connecting;
  }

  async function websocketFetch(
    input: RequestInfo | URL,
    init?: RequestInit,
  ): Promise<Response> {
    const url =
      input instanceof URL
        ? input.toString()
        : typeof input === 'string'
          ? input
          : input.url;

    if (init?.method !== 'POST' || !url.endsWith('/responses')) {
      return globalThis.fetch(input, init);
    }

    let body: Record<string, unknown>;
    try {
      body = JSON.parse(typeof init.body === 'string' ? init.body : '');
    } catch {
      return globalThis.fetch(input, init);
    }

    if (!body.stream) {
      return globalThis.fetch(input, init);
    }

    const headers = normalizeHeaders(init.headers);
    const authorization = headers['authorization'] ?? '';

    const { stream: _, ...requestBody } = body;
    let connection: WebSocket;
    try {
      connection = await getConnection(authorization);
    } catch {
      return fallbackFetch(input, init, requestBody);
    }
    busy = true;

    const encoder = new TextEncoder();

    const responseStream = new ReadableStream<Uint8Array>({
      start(controller) {
        function cleanup() {
          connection.removeEventListener('message', onMessage);
          connection.removeEventListener('error', onError);
          connection.removeEventListener('close', onClose);
          busy = false;
        }

        function onMessage(data: MessageEvent) {
          const text = typeof data.data === 'string' ? data.data : data.data.toString();
          const lines = text.split(/\r?\n/);
          const sseData = lines.map((line: string) => `data: ${line}`).join('\n');
          controller.enqueue(encoder.encode(`${sseData}\n\n`));

          try {
            const event = JSON.parse(text);
            if (
              event.type === 'response.completed' ||
              event.type === 'response.failed' ||
              event.type === 'response.incomplete' ||
              event.type === 'error'
            ) {
              controller.enqueue(encoder.encode('data: [DONE]\n\n'));
              cleanup();
              controller.close();
            }
          } catch {
            // non-JSON frame, continue
          }
        }

        function onError(event: Event) {
          cleanup();
          controller.error(event instanceof Error ? event : new Error('WebSocket error'));
        }

        function onClose() {
          cleanup();
          try {
            controller.close();
          } catch {
            // already closed
          }
        }

        connection.addEventListener('message', onMessage);
        connection.addEventListener('error', onError);
        connection.addEventListener('close', onClose);

        if (init?.signal) {
          if (init.signal.aborted) {
            cleanup();
            controller.error(
              init.signal.reason ??
                new DOMException('Aborted', 'AbortError'),
            );
            return;
          }
          init.signal.addEventListener(
            'abort',
            () => {
              cleanup();
              try {
                controller.error(
                  init!.signal!.reason ??
                    new DOMException('Aborted', 'AbortError'),
                );
              } catch {
                // already closed
              }
            },
            { once: true },
          );
        }

        connection.send(
          JSON.stringify({ type: 'response.create', ...requestBody }),
        );
      },
    });

    return new Response(responseStream, {
      status: 200,
      headers: { 'content-type': 'text/event-stream' },
    });
  }

  return Object.assign(websocketFetch, {
    /** Close the underlying WebSocket connection. */
    close() {
      if (ws) {
        ws.close();
        ws = null;
      }
    },
  });
}

async function fallbackFetch(
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  body: Record<string, unknown>,
) {
  const response = await globalThis.fetch(input, {
    ...init,
    body: JSON.stringify({ ...body, stream: false }),
  });

  if (!response.ok) {
    return response;
  }

  const createResponse = await response.json();
  const threadId =
    typeof createResponse === 'object' &&
    createResponse !== null &&
    'id' in createResponse &&
    typeof createResponse.id === 'string'
      ? createResponse.id
      : null;
  const streamUrl = threadId ? createSseUrl(input, threadId) : null;
  const encoder = new TextEncoder();

  const responseStream = new ReadableStream<Uint8Array>({
    async start(controller) {
      controller.enqueue(encoder.encode(`data: ${JSON.stringify(createResponse)}\n\n`));

      if (!streamUrl) {
        controller.close();
        return;
      }

      const streamResponse = await globalThis.fetch(streamUrl, {
        headers: {
          Accept: 'text/event-stream',
        },
        signal: init?.signal,
      });

      if (!streamResponse.ok) {
        controller.error(
          new Error((await streamResponse.text()) || 'Failed to fetch the chat stream.'),
        );
        return;
      }

      if (!streamResponse.body) {
        controller.error(new Error('The response body is empty.'));
        return;
      }

      const reader = streamResponse.body.getReader();

      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          controller.enqueue(value);
        }
        controller.close();
      } catch (error) {
        controller.error(error);
      } finally {
        reader.releaseLock();
      }
    },
  });

  return new Response(responseStream, {
    status: 200,
    headers: { 'content-type': 'text/event-stream' },
  });
}

function createSseUrl(input: RequestInfo | URL, threadId: string) {
  const url =
    input instanceof URL
      ? new URL(input)
      : typeof input === 'string'
        ? new URL(input)
        : new URL(input.url);

  url.search = '';
  url.searchParams.set('transport', 'sse');
  url.searchParams.set('thread_id', threadId);
  url.hash = '';
  return url.toString();
}

function normalizeHeaders(
  headers: HeadersInit | undefined,
): Record<string, string> {
  const result: Record<string, string> = {};
  if (!headers) return result;

  if (headers instanceof Headers) {
    headers.forEach((v, k) => {
      result[k.toLowerCase()] = v;
    });
  } else if (Array.isArray(headers)) {
    for (const [k, v] of headers) {
      result[k.toLowerCase()] = v;
    }
  } else {
    for (const [k, v] of Object.entries(headers)) {
      if (v != null) result[k.toLowerCase()] = v;
    }
  }

  return result;
}
