import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callPluginRpc, connectPluginEvents } from '../plugin-runtime-client';

type Listener = (event: { data?: unknown }) => void;

class MockWebSocket {
  static instances: MockWebSocket[] = [];

  listeners = new Map<string, Listener[]>();
  sent: string[] = [];
  closed = false;
  constructor(public readonly url: string) {
    MockWebSocket.instances.push(this);
  }

  addEventListener(type: string, listener: Listener) {
    const bucket = this.listeners.get(type) ?? [];
    bucket.push(listener);
    this.listeners.set(type, bucket);
  }

  close() {
    this.closed = true;
    this.emit('close');
  }

  send(payload: string) {
    this.sent.push(payload);
  }

  emit(type: string, data?: unknown) {
    for (const listener of this.listeners.get(type) ?? []) {
      listener({ data });
    }
  }
}

describe('plugin runtime client', () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
  });

  it('sends JSON-RPC methods using the plugin and function name', async () => {
    const pending = callPluginRpc('plugin-a', 'run', { foo: 1 });
    const socket = MockWebSocket.instances[0];

    expect(socket.url).toContain('/v1/plugins/rpc');
    socket.emit('open');
    expect(socket.sent).toHaveLength(1);

    const request = JSON.parse(socket.sent[0]);
    expect(request.method).toBe('plugin-a.run');
    expect(request.params).toEqual({ foo: 1 });

    socket.emit(
      'message',
      JSON.stringify({
        jsonrpc: '2.0',
        id: request.id,
        result: { ok: true },
      }),
    );

    await expect(pending).resolves.toEqual({ ok: true });
  });

  it('passes plugin UI events to the consumer', () => {
    const seen: Array<{ plugin_id: string; topic: string }> = [];
    const close = connectPluginEvents({
      onEvent: (event) => {
        seen.push(event);
      },
    });
    const socket = MockWebSocket.instances[0];
    socket.emit(
      'message',
      JSON.stringify({
        plugin_id: 'plugin-a',
        topic: 'refresh',
        data: { value: 1 },
        ts: 123,
      }),
    );

    expect(seen).toEqual([
      {
        plugin_id: 'plugin-a',
        topic: 'refresh',
        data: { value: 1 },
        ts: 123,
      },
    ]);

    close();
  });
});
