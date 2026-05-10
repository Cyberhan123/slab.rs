export const plugins = {
  header: {
    title: "插件",
    subtitle: "在工作区中运行 Extism 插件",
  },
  search: {
    placeholder: "搜索已安装插件...",
    ariaLabel: "搜索已安装插件",
  },
  alerts: {
    loadFailedTitle: "插件数据加载失败",
  },
  sections: {
    installed: "已安装插件",
  },
  actions: {
    refresh: "刷新",
    import: "导入包",
    stop: "停止",
    enable: "启用",
    launch: "启动",
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
    sourceVersion: "{{sourceKind}} 来源 - v{{version}}",
  },
  card: {
    runtimeIssue: "运行时问题",
  },
  dialogs: {
    import: {
      title: "导入插件包",
      description: "将 .plugin.slab 插件包安装到本地 plugins 目录。",
      packLabel: "插件包",
      selectedDescription: "这个 .plugin.slab 包会解压到受管理的 plugins 目录。",
      emptyDescription: "选择一个 .plugin.slab 文件即可安装并激活这个工作区的插件。",
      submit: "导入插件",
    },
  },
  desktopOnly: {
    title: "插件功能需要 Tauri 桌面运行时",
    description:
      "这个页面管理桌面插件，因此启动和生命周期控制仅在 Tauri 模式下可用。",
  },
  empty: {
    noInstalled: {
      title: "当前没有已安装插件。",
      description:
        "可以先导入 .plugin.slab 插件包，随后这里会显示工作区里的插件。",
    },
    noInstalledMatches: {
      title: "没有匹配的已安装插件",
      description:
        "试试换一个插件名、状态、来源或版本关键词。",
    },
  },
  error: {
    onlyPluginPacks: "只支持 .plugin.slab 插件包。",
  },
  toast: {
    loadFailed: "加载插件数据失败",
    importFailed: "导入插件包失败",
    invalidPlugin: "所选插件无效",
    unknownValidationError: "未知的插件校验错误",
    actionFailed: "无法更新 {{name}}",
    stopped: "已停止 {{name}}",
    enabled: "已启用 {{name}}",
    launched: "已启动 {{name}}",
    disabled: "已禁用 {{name}}",
    imported: "已导入 {{name}}",
  },
} as const;
