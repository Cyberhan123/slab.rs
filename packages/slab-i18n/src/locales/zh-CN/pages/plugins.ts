export const plugins = {
  header: {
    title: "\u63d2\u4ef6",
    subtitle: "\u5728\u5de5\u4f5c\u533a\u4e2d\u8fd0\u884c Extism \u63d2\u4ef6",
  },
  search: {
    placeholder: "\u641c\u7d22\u5df2\u5b89\u88c5\u63d2\u4ef6...",
    ariaLabel: "\u641c\u7d22\u5df2\u5b89\u88c5\u63d2\u4ef6",
  },
  alerts: {
    loadFailedTitle: "\u63d2\u4ef6\u6570\u636e\u52a0\u8f7d\u5931\u8d25",
  },
  sections: {
    installed: "\u5df2\u5b89\u88c5\u63d2\u4ef6",
  },
  actions: {
    refresh: "\u5237\u65b0",
    import: "\u5bfc\u5165\u5305",
    stop: "\u505c\u6b62",
    enable: "\u542f\u7528",
    launch: "\u542f\u52a8",
    disableAria: "\u7981\u7528 {{name}}",
    enableAria: "\u542f\u7528 {{name}}",
  },
  status: {
    working: "\u5904\u7406\u4e2d",
    invalid: "\u65e0\u6548",
    running: "\u8fd0\u884c\u4e2d",
    idle: "\u7a7a\u95f2",
    disabled: "\u5df2\u7981\u7528",
  },
  summary: {
    invalidManifest: "\u63d2\u4ef6\u6e05\u5355\u9700\u8981\u4fee\u590d",
    runtimeActive: "\u63d2\u4ef6\u8fd0\u884c\u65f6\u5f53\u524d\u5904\u4e8e\u6d3b\u52a8\u72b6\u6001",
    disabled: "\u91cd\u65b0\u542f\u7528\u540e\u624d\u53ef\u8fd0\u884c",
    updateReady: "\u5df2\u5b89\u88c5 v{{version}}\uff0c\u53ef\u66f4\u65b0",
    webviewRuntime: "\u5df2\u914d\u7f6e WebView \u548c\u8fd0\u884c\u65f6\u5165\u53e3",
    runtimeHooks: "\u5df2\u63d0\u4f9b\u8fd0\u884c\u65f6\u94a9\u5b50",
    uiEntry: "\u5df2\u914d\u7f6e\u63d2\u4ef6 UI \u5165\u53e3",
    sourceVersion: "{{sourceKind}} \u6765\u6e90 - v{{version}}",
  },
  card: {
    runtimeIssue: "\u8fd0\u884c\u65f6\u95ee\u9898",
  },
  dialogs: {
    import: {
      title: "\u5bfc\u5165\u63d2\u4ef6\u5305",
      description: "\u5c06 .plugin.slab \u63d2\u4ef6\u5305\u5b89\u88c5\u5230\u672c\u5730 plugins \u76ee\u5f55\u3002",
      packLabel: "\u63d2\u4ef6\u5305",
      selectedDescription: "\u8fd9\u4e2a .plugin.slab \u5305\u4f1a\u89e3\u538b\u5230\u53d7\u7ba1\u7406\u7684 plugins \u76ee\u5f55\u3002",
      emptyDescription: "\u9009\u62e9\u4e00\u4e2a .plugin.slab \u6587\u4ef6\u5373\u53ef\u5b89\u88c5\u5e76\u6fc0\u6d3b\u8fd9\u4e2a\u5de5\u4f5c\u533a\u7684\u63d2\u4ef6\u3002",
      submit: "\u5bfc\u5165\u63d2\u4ef6",
    },
  },
  desktopOnly: {
    title: "\u63d2\u4ef6\u529f\u80fd\u9700\u8981 Tauri \u684c\u9762\u8fd0\u884c\u65f6",
    description:
      "\u8fd9\u4e2a\u9875\u9762\u7ba1\u7406\u684c\u9762\u63d2\u4ef6\uff0c\u56e0\u6b64\u542f\u52a8\u548c\u751f\u547d\u5468\u671f\u63a7\u5236\u4ec5\u5728 Tauri \u6a21\u5f0f\u4e0b\u53ef\u7528\u3002",
  },
  empty: {
    noInstalled: {
      title: "\u5f53\u524d\u6ca1\u6709\u5df2\u5b89\u88c5\u63d2\u4ef6\u3002",
      description:
        "\u53ef\u4ee5\u5148\u5bfc\u5165 .plugin.slab \u63d2\u4ef6\u5305\uff0c\u968f\u540e\u8fd9\u91cc\u4f1a\u663e\u793a\u5de5\u4f5c\u533a\u91cc\u7684\u63d2\u4ef6\u3002",
    },
    noInstalledMatches: {
      title: "\u6ca1\u6709\u5339\u914d\u7684\u5df2\u5b89\u88c5\u63d2\u4ef6",
      description:
        "\u8bd5\u8bd5\u6362\u4e00\u4e2a\u63d2\u4ef6\u540d\u3001\u72b6\u6001\u3001\u6765\u6e90\u6216\u7248\u672c\u5173\u952e\u8bcd\u3002",
    },
  },
  error: {
    onlyPluginPacks: "\u53ea\u652f\u6301 .plugin.slab \u63d2\u4ef6\u5305\u3002",
  },
  toast: {
    loadFailed: "\u52a0\u8f7d\u63d2\u4ef6\u6570\u636e\u5931\u8d25",
    importFailed: "\u5bfc\u5165\u63d2\u4ef6\u5305\u5931\u8d25",
    invalidPlugin: "\u6240\u9009\u63d2\u4ef6\u65e0\u6548",
    unknownValidationError: "\u672a\u77e5\u7684\u63d2\u4ef6\u6821\u9a8c\u9519\u8bef",
    actionFailed: "\u65e0\u6cd5\u66f4\u65b0 {{name}}",
    stopped: "\u5df2\u505c\u6b62 {{name}}",
    enabled: "\u5df2\u542f\u7528 {{name}}",
    launched: "\u5df2\u542f\u52a8 {{name}}",
    disabled: "\u5df2\u7981\u7528 {{name}}",
    imported: "\u5df2\u5bfc\u5165 {{name}}",
  },
} as const;
