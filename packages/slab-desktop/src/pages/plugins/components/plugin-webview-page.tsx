import { useCallback, useEffect, useRef, useState } from 'react';
import { useResizeObserver, useWindowEvent } from '@mantine/hooks';
import { clamp } from 'lodash-es';
import { AlertCircle, Loader2 } from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';

import {
  pluginMountView,
  pluginUnmountView,
  pluginUpdateViewBounds,
  type PluginInfo,
  type PluginViewBounds,
} from '@/lib/plugin-host-bridge';

type PluginWebviewPageProps = {
  plugin: PluginInfo;
};

function rectToBounds(rect: DOMRect): PluginViewBounds {
  return {
    x: rect.left,
    y: rect.top,
    width: clamp(rect.width, 1, Number.POSITIVE_INFINITY),
    height: clamp(rect.height, 1, Number.POSITIVE_INFINITY),
  };
}

export function PluginWebviewPage({ plugin }: PluginWebviewPageProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const [observeHostResize, hostRect] = useResizeObserver<HTMLDivElement>();
  const [error, setError] = useState<string | null>(null);
  const [mounted, setMounted] = useState(false);
  const setHostNode = useCallback(
    (node: HTMLDivElement | null) => {
      hostRef.current = node;
      observeHostResize(node);
    },
    [observeHostResize],
  );

  const syncBounds = useCallback(async () => {
    const host = hostRef.current;
    if (!host) return;
    const bounds = rectToBounds(host.getBoundingClientRect());
    await pluginUpdateViewBounds({ pluginId: plugin.id, bounds });
  }, [plugin.id]);

  useWindowEvent('resize', () => {
    void syncBounds();
  });

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

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
  }, [plugin.id]);

  useEffect(() => {
    if (mounted) {
      void syncBounds();
    }
  }, [hostRect.height, hostRect.width, mounted, syncBounds]);

  return (
    <div className="relative h-full w-full overflow-hidden rounded-[24px] border border-border/70 bg-background">
      <div ref={setHostNode} className="absolute inset-0" />
      {!mounted && !error ? (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center text-muted-foreground">
          <Loader2 className="mr-2 size-4 animate-spin" />
          Loading plugin…
        </div>
      ) : null}
      {error ? (
        <div className="absolute inset-0 flex items-center justify-center p-6">
          <Alert variant="destructive" className="max-w-xl">
            <AlertCircle className="size-4" />
            <AlertTitle>Plugin view failed to mount</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        </div>
      ) : null}
    </div>
  );
}
