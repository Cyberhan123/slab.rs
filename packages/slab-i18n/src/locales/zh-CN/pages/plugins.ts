export const plugins = {
  header: {
    title: "插件",
    subtitle: "在工作区中运行 Extism 插件",
  },
  search: {
    placeholder: "搜索已安装插件和插件市场...",
    ariaLabel: "搜索已安装插件和插件市场",
  },
  alerts: {
    loadFailedTitle: "插件数据加载失败",
  },
  sections: {
    installed: "已安装插件",
    market: "插件市场",
  },
  actions: {
    refresh: "刷新",
    stop: "停止",
    enable: "启用",
    launch: "启动",
    update: "更新",
    install: "安装",
    installed: "已安装",
    disableAria: "禁用 {{name}}",
    enableAria: "启用 {{name}}",
  },
  status: {
    working: "处理中",
    invalid: "无效",
    running: "运行中",
    idle: "空闲",
    disabled: "已禁用",
  },
  summary: {
    invalidManifest: "插件清单需要修复",
    runtimeActive: "插件运行时当前处于活动状态",
    disabled: "重新启用后才可运行",
    updateReady: "已安装 v{{version}}，可更新",
    webviewRuntime: "已配置 WebView 和运行时入口",
    runtimeHooks: "已提供运行时钩子",
    uiEntry: "已配置插件 UI 入口",
    sourceVersion: "{{sourceKind}} 来源 · v{{version}}",
  },
  card: {
    runtimeIssue: "运行时问题",
  },
  market: {
    fallbackDescription: "{{id}} · v{{version}}",
  },
  desktopOnly: {
    title: "插件功能需要 Tauri 桌面运行时",
    description: "这个页面管理桌面插件，因此启动和生命周期控制仅在 Tauri 模式下可用。",
  },
  empty: {
    noInstalled: {
      title: "当前没有已安装插件。",
      description: "可以先从下方插件市场安装插件，随后这里会显示工作区里的插件。",
    },
    noInstalledMatches: {
      title: "没有匹配的已安装插件",
      description: "试试换一个插件名、状态、来源或版本关键词。",
    },
    noMarket: {
      title: "当前没有配置插件市场目录。",
      description: "配置好远程目录后，这里会显示可安装或更新的插件条目。",
    },
    noMarketMatches: {
      title: "插件市场没有匹配结果",
      description: "试试换一个插件名、标签、来源或版本关键词。",
    },
  },
  toast: {
    loadFailed: "加载插件数据失败",
    invalidPlugin: "所选插件无效",
    unknownValidationError: "未知的插件校验错误",
    actionFailed: "无法更新 {{name}}",
    stopped: "已停止 {{name}}",
    enabled: "已启用 {{name}}",
    launched: "已启动 {{name}}",
    disabled: "已禁用 {{name}}",
    installed: "已安装 {{name}}",
    updated: "已更新 {{name}}",
  },
} as const;
