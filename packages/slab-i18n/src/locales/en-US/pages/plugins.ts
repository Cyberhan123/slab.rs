export const plugins = {
  header: {
    title: "Plugins",
    subtitle: "Run workspace plugins with Extism runtime",
  },
  search: {
    placeholder: "Search installed plugins and market...",
    ariaLabel: "Search installed plugins and plugin market",
  },
  alerts: {
    loadFailedTitle: "Plugin data failed to load",
  },
  sections: {
    installed: "Installed Plugins",
    market: "Plugin Market",
  },
  actions: {
    refresh: "Refresh",
    stop: "Stop",
    enable: "Enable",
    launch: "Launch",
    update: "Update",
    install: "Install",
    installed: "Installed",
    disableAria: "Disable {{name}}",
    enableAria: "Enable {{name}}",
  },
  status: {
    working: "Working",
    invalid: "Invalid",
    running: "Running",
    idle: "Idle",
    disabled: "Disabled",
  },
  summary: {
    invalidManifest: "Plugin manifest requires attention",
    runtimeActive: "Plugin runtime is active",
    disabled: "Disabled until re-enabled",
    updateReady: "Installed v{{version}}, update ready",
    webviewRuntime: "WebView and runtime entry configured",
    runtimeHooks: "Runtime hooks available",
    uiEntry: "Plugin UI entry configured",
    sourceVersion: "{{sourceKind}} source · v{{version}}",
  },
  card: {
    runtimeIssue: "Runtime issue",
  },
  market: {
    fallbackDescription: "{{id}} · v{{version}}",
  },
  desktopOnly: {
    title: "Plugins require Tauri desktop runtime",
    description:
      "This page manages desktop plugins, so launching and lifecycle controls only work in Tauri mode.",
  },
  empty: {
    noInstalled: {
      title: "No installed plugins found.",
      description: "Install a plugin from the market below to populate this workspace.",
    },
    noInstalledMatches: {
      title: "No installed plugins match",
      description: "Try a different plugin name, status, source, or version.",
    },
    noMarket: {
      title: "No market catalog configured.",
      description: "Remote catalog entries will appear here with install and update controls.",
    },
    noMarketMatches: {
      title: "No catalog matches",
      description: "Try a different plugin name, tag, source, or version.",
    },
  },
  toast: {
    loadFailed: "Failed to load plugin data",
    invalidPlugin: "Selected plugin is invalid",
    unknownValidationError: "Unknown plugin validation error",
    actionFailed: "Could not update {{name}}",
    stopped: "Stopped {{name}}",
    enabled: "Enabled {{name}}",
    launched: "Launched {{name}}",
    disabled: "Disabled {{name}}",
    installed: "Installed {{name}}",
    updated: "Updated {{name}}",
  },
} as const;
