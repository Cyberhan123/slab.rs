import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { RefreshCw, PlugZap, AlertCircle } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@slab/components/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@slab/components/card";
import { Input } from "@slab/components/input";
import { Textarea } from "@slab/components/textarea";
import { ScrollArea } from "@slab/components/scroll-area";
import { Badge } from "@slab/components/badge";
import { usePageHeader } from "@/hooks/use-global-header-meta";
import { isTauri } from "@/hooks/use-tauri";
import { PAGE_HEADER_META } from "@/layouts/header-meta";
import { SERVER_BASE_URL } from "@/lib/config";
import {
  pluginApiRequest,
  pluginCall,
  pluginList,
  pluginMountView,
  pluginOnEvent,
  pluginUnmountView,
  pluginUpdateViewBounds,
  type PluginEventPayload,
  type PluginInfo,
  type PluginViewBounds,
} from "@/lib/plugin-sdk";

export default function Plugins() {
  const isDesktopTauri = isTauri();
  const viewportRef = useRef<HTMLDivElement | null>(null);
  usePageHeader(PAGE_HEADER_META.plugins);

  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedPluginId, setSelectedPluginId] = useState<string>("");
  const [mountedPluginId, setMountedPluginId] = useState<string>("");
  const [events, setEvents] = useState<PluginEventPayload[]>([]);

  const [callFunction, setCallFunction] = useState("run");
  const [callInput, setCallInput] = useState("{}");
  const [callOutput, setCallOutput] = useState("");
  const [apiOutput, setApiOutput] = useState("");

  const selectedPlugin = useMemo(
    () => plugins.find((plugin) => plugin.id === selectedPluginId),
    [plugins, selectedPluginId],
  );

  const mountedPlugin = useMemo(
    () => plugins.find((plugin) => plugin.id === mountedPluginId),
    [plugins, mountedPluginId],
  );

  const readBounds = useCallback((): PluginViewBounds | null => {
    const element = viewportRef.current;
    if (!element) return null;

    const rect = element.getBoundingClientRect();
    if (rect.width < 2 || rect.height < 2) return null;

    return {
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height,
    };
  }, []);

  const refreshPlugins = useCallback(async () => {
    if (!isDesktopTauri) return;
    setLoading(true);

    try {
      const list = await pluginList();
      setPlugins(list);
      if (!selectedPluginId && list.length > 0) {
        setSelectedPluginId(list[0].id);
      }
    } catch (error) {
      toast.error("Failed to load plugins", {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setLoading(false);
    }
  }, [isDesktopTauri, selectedPluginId]);

  const syncMountedBounds = useCallback(async () => {
    if (!isDesktopTauri || !mountedPluginId) return;
    const bounds = readBounds();
    if (!bounds) return;

    try {
      await pluginUpdateViewBounds({
        pluginId: mountedPluginId,
        bounds,
      });
    } catch (error) {
      console.error("failed to update plugin view bounds", error);
    }
  }, [isDesktopTauri, mountedPluginId, readBounds]);

  const handleMountSelected = useCallback(async () => {
    if (!isDesktopTauri) {
      toast.error("Plugin runtime is only available in Tauri desktop mode");
      return;
    }
    if (!selectedPlugin) {
      toast.error("Please select a plugin");
      return;
    }
    if (!selectedPlugin.valid) {
      toast.error("Selected plugin is invalid", {
        description: selectedPlugin.error || "Unknown plugin validation error",
      });
      return;
    }

    const bounds = readBounds();
    if (!bounds) {
      toast.error("Plugin viewport is not ready");
      return;
    }

    try {
      if (mountedPluginId && mountedPluginId !== selectedPlugin.id) {
        await pluginUnmountView({ pluginId: mountedPluginId });
      }

      await pluginMountView({
        pluginId: selectedPlugin.id,
        bounds,
      });
      setMountedPluginId(selectedPlugin.id);
      setEvents([]);
      toast.success(`Mounted plugin ${selectedPlugin.name}`);
    } catch (error) {
      toast.error("Failed to mount plugin", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [isDesktopTauri, selectedPlugin, readBounds, mountedPluginId]);

  const handleUnmount = useCallback(async () => {
    if (!mountedPluginId) return;
    try {
      await pluginUnmountView({ pluginId: mountedPluginId });
      setMountedPluginId("");
      setEvents([]);
    } catch (error) {
      toast.error("Failed to unmount plugin", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [mountedPluginId]);

  const handleCallPlugin = useCallback(async () => {
    if (!mountedPluginId) {
      toast.error("Mount a plugin first");
      return;
    }
    try {
      const result = await pluginCall({
        pluginId: mountedPluginId,
        function: callFunction,
        input: callInput,
      });
      setCallOutput(result.outputText);
    } catch (error) {
      toast.error("Plugin function call failed", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [callFunction, callInput, mountedPluginId]);

  const handleApiProbe = useCallback(async () => {
    try {
      const response = await pluginApiRequest({
        method: "GET",
        path: "/health",
      });
      setApiOutput(JSON.stringify(response, null, 2));
    } catch (error) {
      toast.error("Plugin API request failed", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, []);

  useEffect(() => {
    void refreshPlugins();
  }, [refreshPlugins]);

  useEffect(() => {
    if (!mountedPluginId || !isDesktopTauri) return;

    let unlisten: (() => void) | undefined;
    let cancelled = false;

    void pluginOnEvent(mountedPluginId, (payload) => {
      setEvents((prev) => [payload, ...prev].slice(0, 50));
    }).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [isDesktopTauri, mountedPluginId]);

  useEffect(() => {
    if (!mountedPluginId || !isDesktopTauri) return;
    const element = viewportRef.current;
    if (!element) return;

    const observer = new ResizeObserver(() => {
      void syncMountedBounds();
    });
    observer.observe(element);

    const onResize = () => {
      void syncMountedBounds();
    };
    const onTransitionEnd = () => {
      void syncMountedBounds();
    };
    window.addEventListener("resize", onResize);
    document.addEventListener("transitionend", onTransitionEnd);

    void syncMountedBounds();

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", onResize);
      document.removeEventListener("transitionend", onTransitionEnd);
    };
  }, [isDesktopTauri, mountedPluginId, syncMountedBounds]);

  useEffect(() => {
    return () => {
      if (mountedPluginId) {
        void pluginUnmountView({ pluginId: mountedPluginId });
      }
    };
  }, [mountedPluginId]);

  if (!isDesktopTauri) {
    return (
      <div className="h-full p-6">
        <Card>
          <CardHeader>
            <CardTitle>Plugins require Tauri desktop runtime</CardTitle>
            <CardDescription>
              This page only works in desktop mode because plugin UI is mounted as child webviews.
            </CardDescription>
          </CardHeader>
        </Card>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-4">
      <div className="grid gap-4 xl:grid-cols-[360px_minmax(0,1fr)]">
        <Card className="h-fit">
          <CardHeader>
            <CardTitle className="flex items-center justify-between">
              <span>Plugin Center</span>
              <Button
                size="sm"
                variant="outline"
                onClick={() => void refreshPlugins()}
                disabled={loading}
              >
                <RefreshCw className={`mr-2 h-4 w-4 ${loading ? "animate-spin" : ""}`} />
                Refresh
              </Button>
            </CardTitle>
            <CardDescription>
              Plugins are loaded from workspace <code>plugins/</code>.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {plugins.length === 0 ? (
              <div className="rounded-md border border-dashed p-4 text-sm text-muted-foreground">
                No plugins found.
              </div>
            ) : (
              plugins.map((plugin) => {
                const selected = selectedPluginId === plugin.id;
                return (
                  <button
                    key={plugin.id}
                    type="button"
                    className={`w-full rounded-md border p-3 text-left transition ${
                      selected ? "border-primary bg-primary/5" : "hover:bg-muted/40"
                    }`}
                    onClick={() => setSelectedPluginId(plugin.id)}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        <p className="font-medium leading-none">{plugin.name}</p>
                        <p className="mt-1 text-xs text-muted-foreground">
                          {plugin.id} · v{plugin.version}
                        </p>
                      </div>
                      <Badge variant={plugin.valid ? "default" : "destructive"}>
                        {plugin.valid ? "Ready" : "Invalid"}
                      </Badge>
                    </div>
                    {!plugin.valid && plugin.error ? (
                      <p className="mt-2 text-xs text-destructive">{plugin.error}</p>
                    ) : null}
                  </button>
                );
              })
            )}

            <div className="grid grid-cols-2 gap-2 pt-2">
              <Button onClick={() => void handleMountSelected()} disabled={!selectedPlugin}>
                <PlugZap className="mr-2 h-4 w-4" />
                Mount
              </Button>
              <Button variant="outline" onClick={() => void handleUnmount()} disabled={!mountedPluginId}>
                Unmount
              </Button>
            </div>
          </CardContent>
        </Card>

        <div className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Plugin Viewport</CardTitle>
              <CardDescription>
                {mountedPlugin
                  ? `Mounted: ${mountedPlugin.name} (${mountedPlugin.id})`
                  : "No plugin mounted"}
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div
                ref={viewportRef}
                className="relative h-[420px] rounded-md border border-dashed bg-muted/20"
              >
                {!mountedPluginId ? (
                  <div className="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
                    Select and mount a plugin to render UI here.
                  </div>
                ) : null}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Wasm & API Bridge</CardTitle>
              <CardDescription>
                Call plugin wasm functions and probe host API proxy.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="grid gap-2 md:grid-cols-[220px_minmax(0,1fr)]">
                <Input
                  value={callFunction}
                  onChange={(event) => setCallFunction(event.target.value)}
                  placeholder="Function name"
                />
                <Button onClick={() => void handleCallPlugin()} disabled={!mountedPluginId}>
                  Call Plugin Function
                </Button>
              </div>
              <Textarea
                value={callInput}
                onChange={(event) => setCallInput(event.target.value)}
                className="min-h-[100px] font-mono text-xs"
                placeholder="Input payload passed to Extism function"
              />
              <Textarea
                value={callOutput}
                readOnly
                className="min-h-[100px] font-mono text-xs"
                placeholder="Function output"
              />
              <div className="flex items-center gap-2">
                <Button variant="outline" onClick={() => void handleApiProbe()}>
                  Probe /health
                </Button>
                <span className="text-xs text-muted-foreground">
                  Host proxy target is fixed to <code>{SERVER_BASE_URL}</code>.
                </span>
              </div>
              <Textarea
                value={apiOutput}
                readOnly
                className="min-h-[110px] font-mono text-xs"
                placeholder="API response"
              />
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <AlertCircle className="h-4 w-4" />
                Plugin Events
              </CardTitle>
              <CardDescription>
                Listening on <code>{mountedPluginId ? `plugin://${mountedPluginId}/event` : "plugin://<id>/event"}</code>
              </CardDescription>
            </CardHeader>
            <CardContent>
              <ScrollArea className="h-[220px] rounded-md border p-2">
                {events.length === 0 ? (
                  <p className="p-2 text-xs text-muted-foreground">No events yet.</p>
                ) : (
                  <div className="space-y-2">
                    {events.map((event, index) => (
                      <pre
                        key={`${event.ts}-${index}`}
                        className="rounded bg-muted p-2 text-xs leading-relaxed"
                      >
                        {JSON.stringify(event, null, 2)}
                      </pre>
                    ))}
                  </div>
                )}
              </ScrollArea>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
