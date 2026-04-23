import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertCircle,
  Download,
  PlugZap,
  Power,
  RefreshCw,
  Rocket,
  Square,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@slab/components/badge";
import { Button } from "@slab/components/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@slab/components/card";
import { Input } from "@slab/components/input";
import { ScrollArea } from "@slab/components/scroll-area";
import { Textarea } from "@slab/components/textarea";
import { usePageHeader } from "@/hooks/use-global-header-meta";
import { isTauri } from "@/hooks/use-tauri";
import { PAGE_HEADER_META } from "@/layouts/header-meta";
import {
  disablePlugin,
  enablePlugin,
  installPlugin,
  listMarketPlugins,
  listPlugins,
  removePlugin,
  startPlugin as startPluginState,
  stopPlugin as stopPluginState,
  type PluginMarketRecord,
  type PluginRecord,
} from "@/lib/plugin-market-api";
import {
  pluginApiRequest,
  pluginCall,
  pluginMountView,
  pluginOnEvent,
  pluginRuntimeList,
  pluginUnmountView,
  pluginUpdateViewBounds,
  type PluginEventPayload,
  type PluginViewBounds,
} from "@/lib/plugin-host-bridge";

type PluginEventEntry = PluginEventPayload & {
  eventKey: string;
};

export default function Plugins() {
  const isDesktopTauri = isTauri();
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const nextEventKeyRef = useRef(0);
  usePageHeader(PAGE_HEADER_META.plugins);

  const [plugins, setPlugins] = useState<PluginRecord[]>([]);
  const [marketPlugins, setMarketPlugins] = useState<PluginMarketRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedPluginId, setSelectedPluginId] = useState<string>("");
  const [mountedPluginId, setMountedPluginId] = useState<string>("");
  const [events, setEvents] = useState<PluginEventEntry[]>([]);
  const [busyPluginId, setBusyPluginId] = useState<string | null>(null);

  const [callFunction, setCallFunction] = useState("run");
  const [callInput, setCallInput] = useState("{}");
  const [callOutput, setCallOutput] = useState("");
  const [apiOutput, setApiOutput] = useState("");

  const selectedPlugin = useMemo(
    () => plugins.find((plugin) => plugin.id === selectedPluginId) ?? null,
    [plugins, selectedPluginId],
  );
  const mountedPlugin = useMemo(
    () => plugins.find((plugin) => plugin.id === mountedPluginId) ?? null,
    [plugins, mountedPluginId],
  );
  const mountedPluginHasWasm = mountedPlugin?.hasWasm === true;

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

  const refreshData = useCallback(
    async (silent = false) => {
      if (!isDesktopTauri) return;
      if (!silent) setLoading(true);

      try {
        const [pluginRows, marketRows] = await Promise.all([listPlugins(), listMarketPlugins()]);
        setPlugins(pluginRows);
        setMarketPlugins(marketRows);
        setSelectedPluginId((current) => {
          if (current && pluginRows.some((plugin) => plugin.id === current)) return current;
          return pluginRows[0]?.id ?? "";
        });
      } catch (error) {
        toast.error("Failed to load plugin data", {
          description: error instanceof Error ? error.message : String(error),
        });
      } finally {
        if (!silent) setLoading(false);
      }
    },
    [isDesktopTauri],
  );

  const runAction = useCallback(async (pluginId: string | null, action: () => Promise<void>) => {
    setBusyPluginId(pluginId);
    try {
      await action();
    } finally {
      setBusyPluginId(null);
    }
  }, []);

  const syncMountedBounds = useCallback(async () => {
    if (!mountedPluginId || !isDesktopTauri) return;
    const bounds = readBounds();
    if (!bounds) return;
    try {
      await pluginUpdateViewBounds({ pluginId: mountedPluginId, bounds });
    } catch (error) {
      console.error("failed to update plugin view bounds", error);
    }
  }, [isDesktopTauri, mountedPluginId, readBounds]);

  const handleStopMounted = useCallback(
    async (lastError?: string) => {
      if (!mountedPluginId) return;
      await pluginUnmountView({ pluginId: mountedPluginId });
      await stopPluginState(mountedPluginId, { lastError: lastError ?? null });
      setMountedPluginId("");
      setEvents([]);
      await refreshData(true);
    },
    [mountedPluginId, refreshData],
  );

  const handleLaunchSelected = useCallback(async () => {
    if (!isDesktopTauri) {
      toast.error("Plugin runtime is only available in Tauri desktop mode");
      return;
    }
    if (!selectedPlugin) {
      toast.error("Please select a plugin");
      return;
    }
    if (!selectedPlugin.enabled) {
      toast.error("Enable the plugin before launching it");
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

    await runAction(selectedPlugin.id, async () => {
      try {
        await pluginRuntimeList();

        if (mountedPluginId && mountedPluginId !== selectedPlugin.id) {
          await handleStopMounted();
        }

        await startPluginState(selectedPlugin.id);
        await pluginMountView({ pluginId: selectedPlugin.id, bounds });
        setMountedPluginId(selectedPlugin.id);
        setEvents([]);
        toast.success(`Launched plugin ${selectedPlugin.name}`);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        try {
          await stopPluginState(selectedPlugin.id, { lastError: message });
        } catch {
          // best effort
        }
        toast.error("Failed to launch plugin", { description: message });
      } finally {
        await refreshData(true);
      }
    });
  }, [handleStopMounted, isDesktopTauri, mountedPluginId, readBounds, refreshData, runAction, selectedPlugin]);

  const handleEnableToggle = useCallback(async () => {
    if (!selectedPlugin) return;

    await runAction(selectedPlugin.id, async () => {
      if (selectedPlugin.enabled) {
        if (mountedPluginId === selectedPlugin.id) {
          await handleStopMounted();
        }
        await disablePlugin(selectedPlugin.id);
        toast.success(`Disabled ${selectedPlugin.name}`);
      } else {
        await enablePlugin(selectedPlugin.id);
        toast.success(`Enabled ${selectedPlugin.name}`);
      }
      await refreshData(true);
    });
  }, [handleStopMounted, mountedPluginId, refreshData, runAction, selectedPlugin]);

  const handleRemoveSelected = useCallback(async () => {
    if (!selectedPlugin) return;
    if (!selectedPlugin.removable) {
      toast.error("Only market ZIP installs can be removed");
      return;
    }

    await runAction(selectedPlugin.id, async () => {
      if (mountedPluginId === selectedPlugin.id) {
        await handleStopMounted();
      }
      await removePlugin(selectedPlugin.id);
      toast.success(`Removed ${selectedPlugin.name}`);
      await refreshData(true);
    });
  }, [handleStopMounted, mountedPluginId, refreshData, runAction, selectedPlugin]);

  const handleInstall = useCallback(
    async (marketPlugin: PluginMarketRecord) => {
      await runAction(marketPlugin.id, async () => {
        await installPlugin({
          pluginId: marketPlugin.id,
          sourceId: marketPlugin.sourceId,
          version: marketPlugin.version,
        });
        toast.success(
          marketPlugin.installedVersion && marketPlugin.updateAvailable
            ? `Updated ${marketPlugin.name}`
            : `Installed ${marketPlugin.name}`,
        );
        await refreshData(true);
      });
    },
    [refreshData, runAction],
  );

  const handleCallPlugin = useCallback(async () => {
    if (!mountedPluginId) {
      toast.error("Launch a plugin first");
      return;
    }
    if (!mountedPluginHasWasm) {
      toast.error("Mounted plugin does not provide a WASM runtime");
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
  }, [callFunction, callInput, mountedPluginHasWasm, mountedPluginId]);

  const handleApiProbe = useCallback(async () => {
    try {
      const response = await pluginApiRequest({ method: "GET", path: "/health" });
      setApiOutput(JSON.stringify(response, null, 2));
    } catch (error) {
      toast.error("Plugin API request failed", {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, []);

  useEffect(() => {
    void refreshData();
  }, [refreshData]);

  useEffect(() => {
    if (!mountedPluginId || !isDesktopTauri) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    void pluginOnEvent(mountedPluginId, (payload) => {
      setEvents((prev) => [
        {
          ...payload,
          eventKey: `${payload.pluginId}-${payload.topic}-${payload.ts}-${nextEventKeyRef.current++}`,
        },
        ...prev,
      ].slice(0, 50));
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
              This page manages desktop plugins, so launching and viewport mounting only work in
              Tauri mode.
            </CardDescription>
          </CardHeader>
        </Card>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-4">
      <div className="grid gap-4 xl:grid-cols-[380px_380px_minmax(0,1fr)]">
        <Card className="h-fit">
          <CardHeader>
            <CardTitle className="flex items-center justify-between">
              <span>Installed Plugins</span>
              <Button
                size="sm"
                variant="outline"
                onClick={() => void refreshData()}
                disabled={loading}
              >
                <RefreshCw className={`mr-2 h-4 w-4 ${loading ? "animate-spin" : ""}`} />
                Refresh
              </Button>
            </CardTitle>
            <CardDescription>
              Server-backed plugin lifecycle state is merged with local <code>plugin.json</code>.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {plugins.length === 0 ? (
              <div className="rounded-md border border-dashed p-4 text-sm text-muted-foreground">
                No installed plugins found.
              </div>
            ) : (
              plugins.map((plugin) => {
                const selected = selectedPluginId === plugin.id;
                const mounted = mountedPluginId === plugin.id;
                const busy = busyPluginId === plugin.id;
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
                      <div className="flex flex-wrap justify-end gap-1">
                        <Badge variant={plugin.valid ? "default" : "destructive"}>
                          {plugin.valid ? "Ready" : "Invalid"}
                        </Badge>
                        <Badge variant={plugin.enabled ? "outline" : "secondary"}>
                          {plugin.enabled ? "Enabled" : "Disabled"}
                        </Badge>
                        {mounted ? <Badge variant="outline">Running</Badge> : null}
                        {busy ? <Badge variant="outline">Working</Badge> : null}
                      </div>
                    </div>
                    <div className="mt-2 flex flex-wrap items-center gap-2">
                      <Badge variant="outline">{plugin.sourceKind}</Badge>
                      <Badge variant="outline">{plugin.runtimeStatus}</Badge>
                      {plugin.availableVersion ? (
                        <Badge variant={plugin.updateAvailable ? "default" : "outline"}>
                          market v{plugin.availableVersion}
                        </Badge>
                      ) : null}
                      {plugin.hasWasm ? <Badge variant="outline">WASM runtime</Badge> : null}
                    </div>
                    {plugin.lastError ? (
                      <p className="mt-2 text-xs text-destructive">{plugin.lastError}</p>
                    ) : null}
                  </button>
                );
              })
            )}

            <div className="grid grid-cols-2 gap-2 pt-2">
              <Button
                variant={selectedPlugin?.enabled ? "outline" : "default"}
                onClick={() => void handleEnableToggle()}
                disabled={!selectedPlugin || busyPluginId === selectedPlugin.id}
              >
                <Power className="mr-2 h-4 w-4" />
                {selectedPlugin?.enabled ? "Disable" : "Enable"}
              </Button>
              <Button
                onClick={() => void handleLaunchSelected()}
                disabled={
                  !selectedPlugin ||
                  !selectedPlugin.valid ||
                  !selectedPlugin.enabled ||
                  busyPluginId === selectedPlugin.id
                }
              >
                <Rocket className="mr-2 h-4 w-4" />
                Launch
              </Button>
              <Button
                variant="outline"
                onClick={() => void handleStopMounted()}
                disabled={!mountedPluginId || busyPluginId === mountedPluginId}
              >
                <Square className="mr-2 h-4 w-4" />
                Stop
              </Button>
              <Button
                variant="destructive"
                onClick={() => void handleRemoveSelected()}
                disabled={!selectedPlugin?.removable || busyPluginId === selectedPlugin?.id}
              >
                <Trash2 className="mr-2 h-4 w-4" />
                Remove
              </Button>
            </div>
          </CardContent>
        </Card>

        <Card className="h-fit">
          <CardHeader>
            <CardTitle>Plugin Market</CardTitle>
            <CardDescription>
              Remote catalog entries are merged with local install status and update state.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {marketPlugins.length === 0 ? (
              <div className="rounded-md border border-dashed p-4 text-sm text-muted-foreground">
                No market catalog configured.
              </div>
            ) : (
              marketPlugins.map((plugin) => {
                const busy = busyPluginId === plugin.id;
                return (
                  <div key={`${plugin.sourceId}:${plugin.id}`} className="rounded-md border p-3">
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        <p className="font-medium leading-none">{plugin.name}</p>
                        <p className="mt-1 text-xs text-muted-foreground">
                          {plugin.id} · v{plugin.version}
                        </p>
                      </div>
                      <Badge variant="outline">{plugin.sourceId}</Badge>
                    </div>
                    {plugin.description ? (
                      <p className="mt-2 text-sm text-muted-foreground">{plugin.description}</p>
                    ) : null}
                    <div className="mt-2 flex flex-wrap items-center gap-2">
                      {plugin.installedVersion ? (
                        <Badge variant="outline">installed v{plugin.installedVersion}</Badge>
                      ) : (
                        <Badge variant="secondary">Not installed</Badge>
                      )}
                      {plugin.updateAvailable ? <Badge>Update available</Badge> : null}
                    </div>
                    <div className="mt-3">
                      <Button
                        size="sm"
                        onClick={() => void handleInstall(plugin)}
                        disabled={busy}
                      >
                        <Download className="mr-2 h-4 w-4" />
                        {plugin.installedVersion && plugin.updateAvailable ? "Update" : "Install"}
                      </Button>
                    </div>
                  </div>
                );
              })
            )}
          </CardContent>
        </Card>

        <div className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Runtime Viewport</CardTitle>
              <CardDescription>
                {mountedPlugin
                  ? `Mounted: ${mountedPlugin.name} (${mountedPlugin.id})`
                  : "No plugin mounted"}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {selectedPlugin ? (
                <div className="flex flex-wrap items-center gap-2 text-sm">
                  <Badge variant="outline">{selectedPlugin.sourceKind}</Badge>
                  <Badge variant={selectedPlugin.enabled ? "outline" : "secondary"}>
                    {selectedPlugin.enabled ? "Enabled" : "Disabled"}
                  </Badge>
                  {selectedPlugin.updateAvailable ? <Badge>Update available</Badge> : null}
                  {!selectedPlugin.valid ? (
                    <Badge variant="destructive">
                      <AlertCircle className="mr-1 h-3 w-3" />
                      Invalid
                    </Badge>
                  ) : null}
                </div>
              ) : null}

              <div
                ref={viewportRef}
                className="relative h-[360px] rounded-md border border-dashed bg-muted/20"
              >
                {!mountedPluginId ? (
                  <div className="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
                    Select, enable and launch a plugin to render its UI here.
                  </div>
                ) : null}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Wasm Function Bridge</CardTitle>
              <CardDescription>
                {mountedPluginId
                  ? mountedPluginHasWasm
                    ? "Call Extism functions exposed by the mounted plugin."
                    : "This mounted plugin is WebView-only, so WASM calls are disabled."
                  : "Launch a plugin to call WASM functions."}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="grid gap-2 md:grid-cols-[220px_minmax(0,1fr)]">
                <Input
                  value={callFunction}
                  onChange={(event) => setCallFunction(event.target.value)}
                  placeholder="Function name"
                  disabled={!mountedPluginHasWasm}
                />
                <Button
                  onClick={() => void handleCallPlugin()}
                  disabled={!mountedPluginId || !mountedPluginHasWasm}
                >
                  <PlugZap className="mr-2 h-4 w-4" />
                  Call Plugin Function
                </Button>
              </div>
              <Textarea
                value={callInput}
                onChange={(event) => setCallInput(event.target.value)}
                className="min-h-[100px] font-mono text-xs"
                placeholder="Input payload passed to Extism function"
                disabled={!mountedPluginHasWasm}
              />
              <Textarea
                value={callOutput}
                readOnly
                className="min-h-[100px] font-mono text-xs"
                placeholder="Function output"
              />
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Host API Probe</CardTitle>
              <CardDescription>
                Probe the local host API proxy available to plugin webviews.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <Button variant="outline" onClick={() => void handleApiProbe()} disabled={!mountedPluginId}>
                Probe /health
              </Button>
              <Textarea
                value={apiOutput}
                readOnly
                className="min-h-[120px] font-mono text-xs"
                placeholder="Plugin API response"
              />
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Plugin Events</CardTitle>
              <CardDescription>
                Listening on{" "}
                <code>{mountedPluginId ? `plugin://${mountedPluginId}/event` : "plugin://<id>/event"}</code>
              </CardDescription>
            </CardHeader>
            <CardContent>
              <ScrollArea className="h-[180px] rounded-md border">
                <div className="space-y-2 p-3">
                  {events.length === 0 ? (
                    <p className="text-sm text-muted-foreground">No plugin events received yet.</p>
                  ) : (
                    events.map((event) => (
                      <div key={event.eventKey} className="rounded-md border p-2">
                        <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
                          <span>{event.topic}</span>
                          <span>{new Date(event.ts).toLocaleTimeString()}</span>
                        </div>
                        <pre className="mt-2 overflow-auto text-xs">
                          {JSON.stringify(event.data, null, 2)}
                        </pre>
                      </div>
                    ))
                  )}
                </div>
              </ScrollArea>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}

