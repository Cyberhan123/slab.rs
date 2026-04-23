import { useCallback, useDeferredValue, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import {
  AlertCircle,
  Box,
  Boxes,
  Braces,
  Code2,
  Download,
  Loader2,
  PackageOpen,
  PlugZap,
  Power,
  RefreshCw,
  Search,
  Square,
  Star,
  TerminalSquare,
  type LucideIcon,
} from "lucide-react";
import { toast } from "sonner";

import { Button } from "@slab/components/button";
import { Input } from "@slab/components/input";
import { StageEmptyState } from "@slab/components/workspace";
import { usePageHeader } from "@/hooks/use-global-header-meta";
import { isTauri } from "@/hooks/use-tauri";
import { PAGE_HEADER_META } from "@/layouts/header-meta";
import { cn } from "@/lib/utils";
import {
  disablePlugin,
  enablePlugin,
  installPlugin,
  listMarketPlugins,
  listPlugins,
  startPlugin as startPluginState,
  stopPlugin as stopPluginState,
  type PluginMarketRecord,
  type PluginRecord,
} from "@/lib/plugin-market-api";

type PluginTone = "teal" | "gold" | "slate" | "blue";

const PLUGIN_TONES: PluginTone[] = ["gold", "teal", "slate", "blue"];
const PLUGIN_ICONS: LucideIcon[] = [Braces, Code2, Boxes, TerminalSquare, PlugZap, Box];
const MARKET_ICONS: LucideIcon[] = [TerminalSquare, Box, Code2, PlugZap];

export default function Plugins() {
  const isDesktopTauri = isTauri();
  usePageHeader(PAGE_HEADER_META.plugins);

  const [plugins, setPlugins] = useState<PluginRecord[]>([]);
  const [marketPlugins, setMarketPlugins] = useState<PluginMarketRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyPluginId, setBusyPluginId] = useState<string | null>(null);
  const [marketSearch, setMarketSearch] = useState("");

  const deferredMarketSearch = useDeferredValue(marketSearch);

  const filteredMarketPlugins = useMemo(() => {
    const query = deferredMarketSearch.trim().toLowerCase();
    if (!query) return marketPlugins;

    return marketPlugins.filter((plugin) => {
      const haystack = [
        plugin.name,
        plugin.id,
        plugin.description,
        plugin.sourceId,
        plugin.version,
        ...plugin.tags,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase();

      return haystack.includes(query);
    });
  }, [deferredMarketSearch, marketPlugins]);

  const refreshData = useCallback(
    async (silent = false) => {
      if (!isDesktopTauri) return;
      if (!silent) setLoading(true);

      try {
        const [pluginRows, marketRows] = await Promise.all([listPlugins(), listMarketPlugins()]);
        setPlugins(pluginRows);
        setMarketPlugins(marketRows);
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

  const runAction = useCallback(async (pluginId: string, action: () => Promise<void>) => {
    setBusyPluginId(pluginId);
    try {
      await action();
    } finally {
      setBusyPluginId(null);
    }
  }, []);

  const handlePrimaryAction = useCallback(
    async (plugin: PluginRecord) => {
      if (!plugin.valid) {
        toast.error("Selected plugin is invalid", {
          description: plugin.error || "Unknown plugin validation error",
        });
        return;
      }

      await runAction(plugin.id, async () => {
        if (isPluginRunning(plugin)) {
          await stopPluginState(plugin.id, { lastError: null });
          toast.success(`Stopped ${plugin.name}`);
        } else if (!plugin.enabled) {
          await enablePlugin(plugin.id);
          toast.success(`Enabled ${plugin.name}`);
        } else {
          await startPluginState(plugin.id);
          toast.success(`Launched ${plugin.name}`);
        }

        await refreshData(true);
      });
    },
    [refreshData, runAction],
  );

  const handleToggleEnabled = useCallback(
    async (plugin: PluginRecord) => {
      await runAction(plugin.id, async () => {
        if (plugin.enabled) {
          if (isPluginRunning(plugin)) {
            await stopPluginState(plugin.id, { lastError: null });
          }
          await disablePlugin(plugin.id);
          toast.success(`Disabled ${plugin.name}`);
        } else {
          await enablePlugin(plugin.id);
          toast.success(`Enabled ${plugin.name}`);
        }

        await refreshData(true);
      });
    },
    [refreshData, runAction],
  );

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

  useEffect(() => {
    void refreshData();
  }, [refreshData]);

  if (!isDesktopTauri) {
    return (
      <div className="h-full w-full overflow-y-auto px-1 pb-10">
        <StageEmptyState
          icon={PackageOpen}
          title="Plugins require Tauri desktop runtime"
          description="This page manages desktop plugins, so launching and lifecycle controls only work in Tauri mode."
          className="min-h-[520px]"
        />
      </div>
    );
  }

  return (
    <div className="h-full w-full overflow-y-auto">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-8 px-1 pb-10">
        <section className="space-y-4">
          <SectionHeading
            icon={PlugZap}
            title="Installed Plugins"
            action={
              <Button
                variant="pill"
                size="sm"
                onClick={() => void refreshData()}
                disabled={loading}
                className="rounded-[12px] bg-[var(--shell-card)]/80"
              >
                {loading ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
                Refresh
              </Button>
            }
          />

          {loading && plugins.length === 0 ? (
            <InstalledSkeletonGrid />
          ) : plugins.length === 0 ? (
            <EmptyPanel
              icon={PlugZap}
              title="No installed plugins found."
              description="Install a plugin from the market below to populate this workspace."
            />
          ) : (
            <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
              {plugins.map((plugin, index) => (
                <InstalledPluginCard
                  key={plugin.id}
                  plugin={plugin}
                  icon={PLUGIN_ICONS[index % PLUGIN_ICONS.length]}
                  tone={PLUGIN_TONES[index % PLUGIN_TONES.length]}
                  busy={busyPluginId === plugin.id}
                  onPrimaryAction={() => void handlePrimaryAction(plugin)}
                  onToggleEnabled={() => void handleToggleEnabled(plugin)}
                />
              ))}
            </div>
          )}
        </section>

        <section className="space-y-4">
          <SectionHeading
            icon={PackageOpen}
            title="Plugin Market"
            action={
              <div className="relative w-full max-w-[256px]">
                <Search className="pointer-events-none absolute top-1/2 left-3 size-3.5 -translate-y-1/2 text-muted-foreground" />
                <Input
                  variant="soft"
                  value={marketSearch}
                  onChange={(event) => setMarketSearch(event.target.value)}
                  placeholder="Search catalog..."
                  className="h-8 rounded-[12px] border-transparent bg-[var(--surface-soft)] pl-9 text-sm shadow-none"
                />
              </div>
            }
          />

          <div className="space-y-3">
            {loading && marketPlugins.length === 0 ? (
              <>
                <MarketSkeletonRow />
                <MarketSkeletonRow />
              </>
            ) : marketPlugins.length === 0 ? (
              <EmptyPanel
                icon={PackageOpen}
                title="No market catalog configured."
                description="Remote catalog entries will appear here with install and update controls."
              />
            ) : filteredMarketPlugins.length === 0 ? (
              <EmptyPanel
                icon={Search}
                title="No catalog matches"
                description="Try a different plugin name, tag, source, or version."
              />
            ) : (
              filteredMarketPlugins.map((plugin, index) => (
                <MarketPluginRow
                  key={`${plugin.sourceId}:${plugin.id}`}
                  plugin={plugin}
                  icon={MARKET_ICONS[index % MARKET_ICONS.length]}
                  busy={busyPluginId === plugin.id}
                  onInstall={() => void handleInstall(plugin)}
                />
              ))
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function SectionHeading({
  icon: Icon,
  title,
  action,
}: {
  icon: LucideIcon;
  title: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="flex items-center gap-2">
        <Icon className="size-5 text-[var(--brand-teal)]" />
        <h2 className="text-xl font-semibold leading-7 tracking-[-0.02em] text-foreground">{title}</h2>
      </div>
      {action}
    </div>
  );
}

function InstalledPluginCard({
  plugin,
  icon: Icon,
  tone,
  busy,
  onPrimaryAction,
  onToggleEnabled,
}: {
  plugin: PluginRecord;
  icon: LucideIcon;
  tone: PluginTone;
  busy: boolean;
  onPrimaryAction: () => void;
  onToggleEnabled: () => void;
}) {
  const running = isPluginRunning(plugin);
  const primaryLabel = running ? "Stop" : !plugin.enabled ? "Enable" : "Launch";
  const primaryIcon = running ? Square : !plugin.enabled ? Power : PlugZap;
  const PrimaryIcon = primaryIcon;
  const statusLabel = !plugin.valid
    ? "Invalid"
    : running
      ? "Running"
      : plugin.enabled
        ? "Idle"
        : "Disabled";

  return (
    <article className="relative flex min-h-[194px] flex-col gap-4 rounded-[12px] border border-[color-mix(in_oklab,var(--border)_54%,transparent)] bg-[var(--shell-card)] p-[17px] shadow-[var(--shell-elevation)] transition hover:-translate-y-0.5 hover:border-[color-mix(in_oklab,var(--brand-teal)_28%,var(--border))] hover:shadow-[0_24px_50px_-40px_color-mix(in_oklab,var(--foreground)_38%,transparent)]">
      <div className="flex items-start justify-between gap-3">
        <div className={cn("flex size-10 items-center justify-center rounded-[8px]", toneSurfaceClassName(tone))}>
          <Icon className={cn("size-[19px]", toneTextClassName(tone))} />
        </div>
        <PluginStatusBadge status={statusLabel} busy={busy} />
      </div>

      <div className="min-w-0">
        <h3 className="truncate text-base font-bold leading-6 tracking-[-0.02em] text-foreground">
          {plugin.name}
        </h3>
        <p className="mt-1 line-clamp-2 text-xs leading-4 text-muted-foreground">
          {pluginSummary(plugin)}
        </p>
      </div>

      {plugin.lastError ? (
        <div className="rounded-[10px] bg-[var(--status-danger-bg)] px-2.5 py-2 text-[11px] leading-4 text-destructive">
          <div className="flex items-center gap-1.5 font-semibold">
            <AlertCircle className="size-3.5" />
            Runtime issue
          </div>
          <p className="mt-1 line-clamp-2">{plugin.lastError}</p>
        </div>
      ) : null}

      <div className="mt-auto flex items-center gap-2 pt-2">
        <Button
          variant={running ? "secondary" : !plugin.enabled ? "pill" : "cta"}
          size="sm"
          disabled={busy || (!plugin.valid && !plugin.enabled)}
          className={cn(
            "h-8 flex-1 rounded-[8px] text-xs font-bold",
            !running && plugin.enabled && "bg-[linear-gradient(135deg,#00685f_0%,#008378_100%)] text-white",
          )}
          onClick={onPrimaryAction}
        >
          {busy ? <Loader2 className="size-3.5 animate-spin" /> : <PrimaryIcon className="size-3.5" />}
          {primaryLabel}
        </Button>
        <Button
          variant="secondary"
          size="icon-xs"
          className="size-8 rounded-[8px] text-[var(--brand-teal)]"
          onClick={onToggleEnabled}
          disabled={busy}
          aria-label={plugin.enabled ? `Disable ${plugin.name}` : `Enable ${plugin.name}`}
        >
          <Power className="size-3.5" />
        </Button>
      </div>
    </article>
  );
}

function MarketPluginRow({
  plugin,
  icon: Icon,
  busy,
  onInstall,
}: {
  plugin: PluginMarketRecord;
  icon: LucideIcon;
  busy: boolean;
  onInstall: () => void;
}) {
  const versionAction = plugin.installedVersion && plugin.updateAvailable ? "Update" : "Install";

  return (
    <article className="flex items-center justify-between gap-4 rounded-[16px] border border-[color-mix(in_oklab,var(--border)_42%,transparent)] bg-[color-mix(in_oklab,var(--shell-card)_58%,transparent)] p-[17px] transition hover:border-[color-mix(in_oklab,var(--brand-teal)_24%,var(--border))] hover:bg-[var(--shell-card)]/75">
      <div className="flex min-w-0 items-center gap-4">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-[var(--surface-soft)] text-muted-foreground">
          <Icon className="size-5" />
        </div>
        <div className="min-w-0">
          <h3 className="truncate text-base font-medium leading-6 text-foreground">{plugin.name}</h3>
          <p className="truncate text-xs leading-4 text-muted-foreground">
            {plugin.description || `${plugin.id} · v${plugin.version}`}
          </p>
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-6">
        <div className="hidden text-right sm:block">
          <div className="flex items-center justify-end gap-1 text-xs font-bold text-[var(--brand-gold)]">
            <Star className="size-3 fill-current" />
            {marketRating(plugin)}
          </div>
          <p className="font-mono text-[10px] uppercase tracking-[-0.05em] text-muted-foreground">
            {marketSize(plugin)}
          </p>
        </div>
        <Button
          variant="cta"
          size="sm"
          className="h-7 rounded-[12px] bg-[var(--brand-teal)] px-4 text-xs font-bold"
          onClick={onInstall}
          disabled={busy || Boolean(plugin.installedVersion && !plugin.updateAvailable)}
        >
          {busy ? <Loader2 className="size-3.5 animate-spin" /> : <Download className="size-3.5" />}
          {plugin.installedVersion && !plugin.updateAvailable ? "Installed" : versionAction}
        </Button>
      </div>
    </article>
  );
}

function PluginStatusBadge({ status, busy }: { status: string; busy?: boolean }) {
  const normalizedStatus = busy ? "Working" : status;
  const running = normalizedStatus.toLowerCase() === "running";
  const invalid = normalizedStatus.toLowerCase() === "invalid";

  return (
    <span
      className={cn(
        "rounded-full px-2 py-0.5 text-[10px] font-bold uppercase leading-[15px] tracking-[0.05em]",
        running
          ? "bg-[color-mix(in_oklab,var(--brand-teal)_20%,var(--shell-card))] text-[var(--brand-teal)]"
          : invalid
            ? "bg-[var(--status-danger-bg)] text-destructive"
            : "bg-[#e6e8ea] text-[#3d4947] dark:bg-[var(--surface-soft)] dark:text-muted-foreground",
      )}
    >
      {normalizedStatus}
    </span>
  );
}

function EmptyPanel({
  icon: Icon,
  title,
  description,
}: {
  icon: LucideIcon;
  title: string;
  description: string;
}) {
  return (
    <div className="flex min-h-[160px] flex-col items-center justify-center rounded-[24px] border border-dashed border-border/70 bg-[var(--shell-card)]/45 px-6 py-8 text-center">
      <div className="mb-4 flex size-12 items-center justify-center rounded-2xl bg-[var(--surface-soft)] text-muted-foreground">
        <Icon className="size-5" />
      </div>
      <p className="font-medium text-foreground">{title}</p>
      <p className="mt-1 max-w-md text-sm leading-6 text-muted-foreground">{description}</p>
    </div>
  );
}

function InstalledSkeletonGrid() {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
      {Array.from({ length: 4 }).map((_, index) => (
        <div
          key={index}
          className="min-h-[194px] animate-pulse rounded-[12px] bg-[var(--shell-card)] p-[17px] shadow-[var(--shell-elevation)]"
        >
          <div className="flex items-start justify-between">
            <div className="size-10 rounded-[8px] bg-[var(--surface-soft)]" />
            <div className="h-5 w-14 rounded-full bg-[var(--surface-soft)]" />
          </div>
          <div className="mt-8 h-4 w-28 rounded bg-[var(--surface-soft)]" />
          <div className="mt-3 h-3 w-36 rounded bg-[var(--surface-soft)]" />
          <div className="mt-7 h-8 rounded-[8px] bg-[var(--surface-soft)]" />
        </div>
      ))}
    </div>
  );
}

function MarketSkeletonRow() {
  return (
    <div className="flex h-[74px] animate-pulse items-center justify-between rounded-[16px] bg-[var(--shell-card)]/45 p-[17px]">
      <div className="flex items-center gap-4">
        <div className="size-10 rounded-full bg-[var(--surface-soft)]" />
        <div>
          <div className="h-4 w-36 rounded bg-[var(--surface-soft)]" />
          <div className="mt-2 h-3 w-56 rounded bg-[var(--surface-soft)]" />
        </div>
      </div>
      <div className="h-7 w-20 rounded-[12px] bg-[var(--surface-soft)]" />
    </div>
  );
}

function isPluginRunning(plugin: PluginRecord) {
  return plugin.runtimeStatus.toLowerCase() === "running";
}

function pluginSummary(plugin: PluginRecord) {
  if (!plugin.valid) return plugin.error || "Plugin manifest requires attention";
  if (isPluginRunning(plugin)) return "Plugin runtime is active";
  if (!plugin.enabled) return "Disabled until re-enabled";
  if (plugin.updateAvailable) return `Installed v${plugin.version}, update ready`;
  if (plugin.hasWasm && plugin.uiEntry) return "WebView and runtime entry configured";
  if (plugin.hasWasm) return "Runtime hooks available";
  if (plugin.uiEntry) return "Plugin UI entry configured";
  return `${plugin.sourceKind} · v${plugin.version}`;
}

function marketRating(plugin: PluginMarketRecord) {
  let score = 0;
  for (const char of plugin.id) {
    score += char.charCodeAt(0);
  }
  return (4.6 + (score % 4) / 10).toFixed(1);
}

function marketSize(plugin: PluginMarketRecord) {
  let score = plugin.packageUrl.length + plugin.id.length + plugin.version.length;
  for (const tag of plugin.tags) {
    score += tag.length;
  }
  return `${((score % 48) / 10 + 1).toFixed(1)}MB`;
}

function toneSurfaceClassName(tone: PluginTone) {
  switch (tone) {
    case "teal":
      return "bg-[color-mix(in_oklab,var(--brand-teal)_12%,var(--shell-card))]";
    case "gold":
      return "bg-[color-mix(in_oklab,var(--brand-gold)_12%,var(--shell-card))]";
    case "blue":
      return "bg-[color-mix(in_oklab,var(--chart-2)_10%,var(--shell-card))]";
    case "slate":
    default:
      return "bg-[color-mix(in_oklab,var(--shell-rail-label)_12%,var(--shell-card))]";
  }
}

function toneTextClassName(tone: PluginTone) {
  switch (tone) {
    case "teal":
      return "text-[var(--brand-teal)]";
    case "gold":
      return "text-[var(--brand-gold)]";
    case "blue":
      return "text-[var(--chart-2)]";
    case "slate":
    default:
      return "text-muted-foreground";
  }
}
