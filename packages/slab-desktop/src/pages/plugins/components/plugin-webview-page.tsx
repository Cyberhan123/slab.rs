import { useCallback, useEffect, useRef, useState } from 'react';
import { useResizeObserver, useWindowEvent } from '@mantine/hooks';
import { clamp } from 'lodash-es';
import { AlertCircle, Loader2 } from 'lucide-react';

import type { SlabApiBridgeRequest, SlabApiBridgeResponse } from '@slab/api/plugin';
import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';

import { isTauri } from '@/hooks/use-tauri';
import {
  pluginMountView,
  pluginUnmountView,
  pluginUpdateViewBounds,
  type PluginViewBounds,
} from '@/lib/plugin-host-bridge';
import type { PluginRecord } from '../utils';

const PLUGIN_SDK_MESSAGE_SOURCE = 'slab-plugin-sdk';
const PLUGIN_HOST_MESSAGE_SOURCE = 'slab-plugin-host';
const PLUGIN_API_BRIDGE_UNAVAILABLE =
  'Plugin API bridge is only available in the desktop plugin WebView host.';

type PluginBridgeRequestMessage = {
  source: typeof PLUGIN_SDK_MESSAGE_SOURCE;
  type: 'api.request';
  id: string;
  request: SlabApiBridgeRequest;
};

type PluginBridgeResponseMessage =
  | {
      source: typeof PLUGIN_HOST_MESSAGE_SOURCE;
      type: 'api.response';
      id: string;
      ok: true;
      response: SlabApiBridgeResponse;
    }
  | {
      source: typeof PLUGIN_HOST_MESSAGE_SOURCE;
      type: 'api.response';
      id: string;
      ok: false;
      error: string;
    };

type PluginWebviewPageProps = {
  plugin: PluginRecord;
};

function rectToBounds(rect: DOMRect): PluginViewBounds {
  return {
    x: rect.left,
    y: rect.top,
    width: clamp(rect.width, 1, Number.POSITIVE_INFINITY),
    height: clamp(rect.height, 1, Number.POSITIVE_INFINITY),
  };
}

function readBridgeRequestMessage(data: unknown): PluginBridgeRequestMessage | null {
  if (!data || typeof data !== 'object') return null;
  const record = data as Record<string, unknown>;
  if (
    record.source !== PLUGIN_SDK_MESSAGE_SOURCE ||
    record.type !== 'api.request' ||
    typeof record.id !== 'string' ||
    !record.request ||
    typeof record.request !== 'object'
  ) {
    return null;
  }

  const request = record.request as Record<string, unknown>;
  if (typeof request.method !== 'string' || typeof request.path !== 'string') {
    return null;
  }

  return record as PluginBridgeRequestMessage;
}

export function PluginWebviewPage({ plugin }: PluginWebviewPageProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const [observeHostResize, hostRect] = useResizeObserver<HTMLDivElement>();
  const [error, setError] = useState<string | null>(null);
  const [mounted, setMounted] = useState(false);
  const isDesktopTauri = isTauri();
  const browserMissingUrlError =
    !isDesktopTauri && !plugin.uiUrl ? 'Plugin is missing a browser UI URL.' : null;
  const visibleError = error ?? browserMissingUrlError;

  const setHostNode = useCallback(
    (node: HTMLDivElement | null) => {
      hostRef.current = node;
      observeHostResize(node);
    },
    [observeHostResize],
  );

  const syncBounds = useCallback(async () => {
    if (!isDesktopTauri) return;
    const host = hostRef.current;
    if (!host) return;
    const bounds = rectToBounds(host.getBoundingClientRect());
    await pluginUpdateViewBounds({ pluginId: plugin.id, bounds });
  }, [isDesktopTauri, plugin.id]);

  useWindowEvent('resize', () => {
    void syncBounds();
  });

  useEffect(() => {
    setMounted(false);
    setError(null);
  }, [isDesktopTauri, plugin.id, plugin.uiUrl]);

  useEffect(() => {
    if (!isDesktopTauri) return undefined;
    const host = hostRef.current;
    if (!host) return undefined;

    let cancelled = false;
    const mount = async () => {
      try {
        const bounds = rectToBounds(host.getBoundingClientRect());
        await pluginMountView({ pluginId: plugin.id, bounds });
        if (!cancelled) {
          setMounted(true);
          setError(null);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      }
    };

    void mount();

    return () => {
      cancelled = true;
      void pluginUnmountView({ pluginId: plugin.id });
    };
  }, [isDesktopTauri, plugin.id]);

  useEffect(() => {
    if (mounted) {
      void syncBounds();
    }
  }, [hostRect.height, hostRect.width, mounted, syncBounds]);

  useEffect(() => {
    if (isDesktopTauri || !plugin.uiUrl) return undefined;
    const pluginWindow = iframeRef.current?.contentWindow;
    if (!pluginWindow) return undefined;

    const handleMessage = (event: MessageEvent) => {
      if (event.source !== pluginWindow) return;
      const message = readBridgeRequestMessage(event.data);
      if (!message) return;

      const sendResponse = (response: PluginBridgeResponseMessage) => {
        pluginWindow.postMessage(response, '*');
      };

      sendResponse({
        source: PLUGIN_HOST_MESSAGE_SOURCE,
        type: 'api.response',
        id: message.id,
        ok: false,
        error: PLUGIN_API_BRIDGE_UNAVAILABLE,
      });
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, [isDesktopTauri, plugin.uiUrl]);

  return (
    <div
      className="relative h-full w-full overflow-hidden rounded-2xl border border-border/70 bg-background"
      data-testid={`plugin-view-${plugin.id}`}
    >
      {isDesktopTauri ? (
        <div ref={setHostNode} className="absolute inset-0" />
      ) : plugin.uiUrl ? (
        <iframe
          ref={iframeRef}
          title={plugin.name}
          src={plugin.uiUrl}
          sandbox="allow-scripts allow-forms"
          className="absolute inset-0 h-full w-full border-0"
          onLoad={() => setMounted(true)}
          data-testid={`plugin-frame-${plugin.id}`}
        />
      ) : null}
      {!mounted && !visibleError ? (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center text-muted-foreground">
          <Loader2 className="mr-2 size-4 animate-spin" />
          Loading plugin...
        </div>
      ) : null}
      {visibleError ? (
        <div className="absolute inset-0 flex items-center justify-center p-6">
          <Alert variant="destructive" className="max-w-xl">
            <AlertCircle className="size-4" />
            <AlertTitle>Plugin view failed to mount</AlertTitle>
            <AlertDescription>{visibleError}</AlertDescription>
          </Alert>
        </div>
      ) : null}
    </div>
  );
}
