import { useCallback } from 'react';

import { SERVER_BASE_URL } from '@slab/api/config';

export type PluginEventPayload = {
  plugin_id: string;
  topic: string;
  data: unknown;
  ts: number;
};

type JsonRpcSuccess<T> = {
  jsonrpc: '2.0';
  id: string;
  result: T;
};

type JsonRpcFailure = {
  jsonrpc: '2.0';
  id: string;
  error: {
    code: number;
    message: string;
    data?: unknown;
  };
};

type PluginEventConnectionOptions = {
  onEvent: (event: PluginEventPayload) => void;
  onError?: (error: Event) => void;
};

function webSocketUrl(path: string) {
  const endpoint = new URL(SERVER_BASE_URL);
  endpoint.protocol = endpoint.protocol === 'https:' ? 'wss:' : 'ws:';
  endpoint.pathname = path;
  endpoint.search = '';
  endpoint.hash = '';
  return endpoint.toString();
}

export function connectPluginEvents({ onEvent, onError }: PluginEventConnectionOptions) {
  if (typeof WebSocket === 'undefined') {
    return () => {};
  }

  const socket = new WebSocket(webSocketUrl('/v1/plugins/events'));
  socket.addEventListener('message', (message) => {
    const event = parsePluginEventPayload(String(message.data));
    if (event) {
      onEvent(event);
    }
  });
  if (onError) {
    socket.addEventListener('error', onError);
  }

  return () => socket.close();
}

export function callPluginRpc<T = unknown>(
  pluginId: string,
  functionName: string,
  params?: unknown,
) {
  const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const method = `${pluginId}.${functionName}`;

  return new Promise<T>((resolve, reject) => {
    if (typeof WebSocket === 'undefined') {
      reject(new Error('plugin RPC websocket is not available'));
      return;
    }

    const socket = new WebSocket(webSocketUrl('/v1/plugins/rpc'));
    let settled = false;
    socket.addEventListener('open', () => {
      socket.send(
        JSON.stringify({
          jsonrpc: '2.0',
          id,
          method,
          params: params ?? null,
        }),
      );
    });
    socket.addEventListener('message', (message) => {
      const response = parseJsonRpcResponse<T>(String(message.data));
      if (!response || response.id !== id) {
        return;
      }

      settled = true;
      socket.close();
      if ('error' in response) {
        reject(new Error(`plugin RPC error ${response.error.code}: ${response.error.message}`));
        return;
      }

      resolve(response.result);
    });
    socket.addEventListener('error', () => {
      settled = true;
      reject(new Error('plugin RPC websocket failed'));
    });
    socket.addEventListener('close', () => {
      if (settled) {
        return;
      }
      reject(new Error('plugin RPC websocket closed before a matching response'));
    }, { once: true });
  });
}

export function usePluginRpcCall<T = unknown>(
  pluginId: string,
  functionName: string,
  params?: unknown,
) {
  return useCallback(
    () => callPluginRpc<T>(pluginId, functionName, params),
    [functionName, params, pluginId],
  );
}

function parsePluginEventPayload(value: string): PluginEventPayload | null {
  try {
    const payload = JSON.parse(value) as Partial<PluginEventPayload>;
    if (
      typeof payload.plugin_id === 'string' &&
      typeof payload.topic === 'string' &&
      typeof payload.ts === 'number'
    ) {
      return {
        plugin_id: payload.plugin_id,
        topic: payload.topic,
        data: payload.data,
        ts: payload.ts,
      };
    }
  } catch {
    return null;
  }

  return null;
}

function parseJsonRpcResponse<T>(value: string): JsonRpcSuccess<T> | JsonRpcFailure | null {
  try {
    const response = JSON.parse(value) as Partial<JsonRpcSuccess<T> & JsonRpcFailure>;
    if (response.jsonrpc !== '2.0' || typeof response.id !== 'string') {
      return null;
    }

    if (response.error) {
      return response as JsonRpcFailure;
    }

    return response as JsonRpcSuccess<T>;
  } catch {
    return null;
  }
}
