import { Channel, invoke } from '@tauri-apps/api/core';

import { isTauri } from '@/hooks/use-tauri';

/**
 * An event emitted on the Tauri IPC channel for streaming chat completions.
 * Matches the `ChatStreamEvent` enum defined in the Rust handler.
 */
type ChatStreamEvent =
  | { type: 'data'; data: string }
  | { type: 'done' }
  | { type: 'error'; message: string };

/**
 * A fetch-compatible adapter that routes POST /v1/chat/completions and
 * POST /v1/completions through the native Tauri IPC channel when running
 * inside the Tauri desktop host.  The response is shaped to look like a
 * `text/event-stream` (SSE) response so that @ant-design/x-sdk XRequest
 * processes it transparently.
 *
 * Falls back to the native fetch API for all other URLs and when the app
 * is running in a plain browser context.
 */
export const tauriStreamingFetch: typeof fetch = (input, init) => {
  if (!isTauri()) {
    return fetch(input, init);
  }

  const request = new Request(input, init);
  const url = new URL(request.url);

  let command: string;
  if (request.method === 'POST' && url.pathname === '/v1/chat/completions') {
    command = 'chat_completions_stream';
  } else {
    return fetch(request);
  }

  return request
    .json()
    .catch(() => ({}))
    .then((body: unknown) => {
      const encoder = new TextEncoder();
      let streamController: ReadableStreamDefaultController<Uint8Array> | null = null;

      const readable = new ReadableStream<Uint8Array>({
        start(controller) {
          streamController = controller;
        },
      });

      const channel = new Channel<ChatStreamEvent>();
      channel.onmessage = (event) => {
        if (!streamController) return;
        try {
          if (event.type === 'data') {
            streamController.enqueue(encoder.encode(`data: ${event.data}\n\n`));
          } else if (event.type === 'done') {
            streamController.enqueue(encoder.encode('data: [DONE]\n\n'));
            streamController.close();
          } else if (event.type === 'error') {
            streamController.error(new Error(event.message));
          }
        } catch {
          // Stream may have already been closed or errored.
        }
      };

      invoke(command, { req: body, onEvent: channel }).catch((err: unknown) => {
        if (!streamController) return;
        try {
          const message =
            typeof err === 'string'
              ? err
              : err instanceof Error
                ? err.message
                : 'IPC error';
          streamController.error(new Error(message));
        } catch {
          // Stream may have already been closed.
        }
      });

      return new Response(readable, {
        status: 200,
        headers: { 'content-type': 'text/event-stream' },
      });
    });
};

type TauriHttpMethod = 'DELETE' | 'GET' | 'POST' | 'PUT';

interface TauriErrorPayload {
  code: number;
  data?: unknown;
  message: string;
  status?: number;
}

interface RouteContext {
  body: unknown;
  pathParams: Record<string, string>;
  query: Record<string, unknown>;
}

interface TauriRouteDefinition {
  buildArgs?: (context: RouteContext) => Record<string, unknown> | undefined;
  command: string;
  method: TauriHttpMethod;
  pattern: string;
  status?: number;
}

const JSON_HEADERS = { 'content-type': 'application/json' };

const TAURI_ROUTE_DEFINITIONS: readonly TauriRouteDefinition[] = [
  {
    method: 'POST',
    pattern: '/v1/audio/transcriptions',
    command: 'transcribe',
    status: 202,
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'GET',
    pattern: '/v1/backends',
    command: 'list_backends',
  },
  {
    method: 'GET',
    pattern: '/v1/chat/models',
    command: 'list_chat_models',
  },
  {
    method: 'POST',
    pattern: '/v1/backends/download',
    command: 'download_backend_lib',
    status: 202,
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'POST',
    pattern: '/v1/backends/reload',
    command: 'reload_backend_lib',
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'GET',
    pattern: '/v1/backends/status',
    command: 'backend_status',
    buildArgs: ({ query }) => ({ query }),
  },
  {
    method: 'GET',
    pattern: '/v1/models',
    command: 'list_models',
    buildArgs: ({ query }) => ({ query }),
  },
  {
    method: 'POST',
    pattern: '/v1/models',
    command: 'create_model',
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'POST',
    pattern: '/v1/models/download',
    command: 'download_model',
    status: 202,
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'POST',
    pattern: '/v1/models/import',
    command: 'import_model_config',
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'POST',
    pattern: '/v1/models/load',
    command: 'load_model',
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'GET',
    pattern: '/v1/models/available',
    command: 'list_available_models',
    buildArgs: ({ query }) => ({ query }),
  },
  {
    method: 'POST',
    pattern: '/v1/models/switch',
    command: 'switch_model',
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'GET',
    pattern: '/v1/models/{id}',
    command: 'get_model',
    buildArgs: ({ pathParams }) => ({ id: pathParams.id }),
  },
  {
    method: 'PUT',
    pattern: '/v1/models/{id}',
    command: 'update_model',
    buildArgs: ({ body, pathParams }) => ({ id: pathParams.id, req: body }),
  },
  {
    method: 'DELETE',
    pattern: '/v1/models/{id}',
    command: 'delete_model',
    buildArgs: ({ pathParams }) => ({ id: pathParams.id }),
  },
  {
    method: 'GET',
    pattern: '/v1/sessions',
    command: 'list_sessions',
  },
  {
    method: 'POST',
    pattern: '/v1/sessions',
    command: 'create_session',
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'DELETE',
    pattern: '/v1/sessions/{id}',
    command: 'delete_session',
    buildArgs: ({ pathParams }) => ({ id: pathParams.id }),
  },
  {
    method: 'GET',
    pattern: '/v1/sessions/{id}/messages',
    command: 'list_session_messages',
    buildArgs: ({ pathParams }) => ({ id: pathParams.id }),
  },
  {
    method: 'GET',
    pattern: '/v1/settings',
    command: 'list_settings',
  },
  {
    method: 'GET',
    pattern: '/v1/settings/{pmid}',
    command: 'get_setting',
    buildArgs: ({ pathParams }) => ({ pmid: pathParams.pmid }),
  },
  {
    method: 'PUT',
    pattern: '/v1/settings/{pmid}',
    command: 'update_setting',
    buildArgs: ({ body, pathParams }) => ({ pmid: pathParams.pmid, body }),
  },
  {
    method: 'POST',
    pattern: '/v1/setup/complete',
    command: 'complete_setup',
    buildArgs: ({ body }) => ({ req: body }),
  },
  {
    method: 'POST',
    pattern: '/v1/setup/ffmpeg/download',
    command: 'download_ffmpeg',
    status: 202,
  },
  {
    method: 'GET',
    pattern: '/v1/setup/status',
    command: 'setup_status',
  },
  {
    method: 'GET',
    pattern: '/v1/system/gpu',
    command: 'gpu_status',
  },
  {
    method: 'GET',
    pattern: '/v1/tasks/{id}/result',
    command: 'get_task_result',
    buildArgs: ({ pathParams }) => ({ id: pathParams.id }),
  },
  {
    method: 'POST',
    pattern: '/v1/tasks/{id}/cancel',
    command: 'cancel_task',
    buildArgs: ({ pathParams }) => ({ id: pathParams.id }),
  },
  {
    method: 'GET',
    pattern: '/v1/tasks/{id}',
    command: 'get_task',
    buildArgs: ({ pathParams }) => ({ id: pathParams.id }),
  },
  {
    method: 'GET',
    pattern: '/v1/tasks',
    command: 'list_tasks',
    buildArgs: ({ query }) => ({ query }),
  },
];

export const tauriAwareFetch: typeof fetch = async (input, init) => {
  const request = new Request(input, init);

  if (!isTauri()) {
    return fetch(request);
  }

  const url = new URL(request.url);
  const routeMatch = matchTauriRoute(request.method, url.pathname);

  if (!routeMatch) {
    return fetch(request);
  }

  const query = toQueryObject(url.searchParams);
  const body = await readJsonBody(request);

  try {
    const args = routeMatch.route.buildArgs?.({
      body,
      pathParams: routeMatch.pathParams,
      query,
    });

    const result = args
      ? await invoke(routeMatch.route.command, args)
      : await invoke(routeMatch.route.command);

    return new Response(JSON.stringify(result), {
      headers: JSON_HEADERS,
      status: routeMatch.route.status ?? 200,
    });
  } catch (error) {
    const normalizedError = normalizeTauriError(error);
    return new Response(JSON.stringify(normalizedError.body), {
      headers: JSON_HEADERS,
      status: normalizedError.status,
    });
  }
};

function normalizePath(pathname: string): string {
  if (pathname.length > 1 && pathname.endsWith('/')) {
    return pathname.slice(0, -1);
  }

  return pathname;
}

function matchTauriRoute(method: string, pathname: string) {
  const normalizedMethod = method.toUpperCase() as TauriHttpMethod;
  const normalizedPath = normalizePath(pathname);

  for (const route of TAURI_ROUTE_DEFINITIONS) {
    if (route.method !== normalizedMethod) {
      continue;
    }

    const pathParams = matchPath(route.pattern, normalizedPath);
    if (pathParams) {
      return { pathParams, route };
    }
  }

  return null;
}

function matchPath(pattern: string, pathname: string): Record<string, string> | null {
  const patternSegments = normalizePath(pattern).split('/').filter(Boolean);
  const pathSegments = pathname.split('/').filter(Boolean);

  if (patternSegments.length !== pathSegments.length) {
    return null;
  }

  const pathParams: Record<string, string> = {};

  for (let index = 0; index < patternSegments.length; index += 1) {
    const patternSegment = patternSegments[index];
    const pathSegment = pathSegments[index];

    if (patternSegment.startsWith('{') && patternSegment.endsWith('}')) {
      pathParams[patternSegment.slice(1, -1)] = decodeURIComponent(pathSegment);
      continue;
    }

    if (patternSegment !== pathSegment) {
      return null;
    }
  }

  return pathParams;
}

function toQueryObject(searchParams: URLSearchParams): Record<string, unknown> {
  const query: Record<string, unknown> = {};

  for (const [key, value] of searchParams.entries()) {
    const existingValue = query[key];
    if (existingValue === undefined) {
      query[key] = value;
      continue;
    }

    query[key] = Array.isArray(existingValue)
      ? [...existingValue, value]
      : [existingValue, value];
  }

  return query;
}

async function readJsonBody(request: Request): Promise<unknown> {
  if (request.method === 'GET' || request.method === 'HEAD') {
    return undefined;
  }

  const rawBody = await request.text();
  if (!rawBody) {
    return undefined;
  }

  try {
    return JSON.parse(rawBody) as unknown;
  } catch {
    return rawBody;
  }
}

function normalizeTauriError(error: unknown): { body: TauriErrorPayload; status: number } {
  const payload = extractStructuredPayload(error);
  if (payload) {
    return {
      body: {
        code: payload.code,
        data: payload.data,
        message: payload.message,
      },
      status: payload.status ?? statusFromCode(payload.code),
    };
  }

  const message = extractErrorMessage(error);
  return {
    body: {
      code: 5002,
      data: undefined,
      message,
    },
    status: 500,
  };
}

function extractStructuredPayload(error: unknown): TauriErrorPayload | null {
  const candidates = [error];

  if (typeof error === 'object' && error !== null) {
    const record = error as Record<string, unknown>;
    candidates.push(record.message, record.error, record.cause);
  }

  for (const candidate of candidates) {
    const parsedPayload = parseErrorPayload(candidate);
    if (parsedPayload) {
      return parsedPayload;
    }
  }

  return null;
}

function parseErrorPayload(value: unknown): TauriErrorPayload | null {
  if (typeof value === 'string') {
    try {
      const parsed = JSON.parse(value) as unknown;
      return parseErrorPayload(parsed);
    } catch {
      return null;
    }
  }

  if (typeof value !== 'object' || value === null) {
    return null;
  }

  const record = value as Record<string, unknown>;
  if (typeof record.code !== 'number' || typeof record.message !== 'string') {
    return null;
  }

  return {
    code: record.code,
    data: record.data,
    message: record.message,
    status: typeof record.status === 'number' ? record.status : undefined,
  };
}

function extractErrorMessage(error: unknown): string {
  if (typeof error === 'string') {
    return error;
  }

  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === 'object' && error !== null) {
    const record = error as Record<string, unknown>;

    if (typeof record.message === 'string') {
      return record.message;
    }

    if (typeof record.error === 'string') {
      return record.error;
    }
  }

  return 'An unexpected error occurred.';
}

function statusFromCode(code: number): number {
  switch (code) {
    case 4000:
      return 400;
    case 4004:
      return 404;
    case 4029:
      return 429;
    case 5003:
      return 503;
    case 5010:
      return 501;
    default:
      return 500;
  }
}
