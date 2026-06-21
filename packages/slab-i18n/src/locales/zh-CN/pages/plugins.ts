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
    installFromUrl: "从 URL 安装",
    stop: "停止",
    enable: "启用",
    launch: "启动",
    update: "更新",
    uninstallAria: "卸载 {{name}}",
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
      uploading: "正在上传插件包",
      cancelUpload: "取消上传",
    },
    urlInstall: {
      title: "从 URL 安装插件",
      description: "直接从包地址安装插件包。",
      pluginId: "插件 ID",
      packageUrl: "包地址",
      packageSha256: "包 SHA256",
      version: "版本",
      submit: "安装插件",
    },
  },
  permissions: {
    reviewTitle: "申请的权限",
    reviewDescription: "在安装前查看这个插件可以执行的操作。",
    reviewedCheckbox: "我已查看该插件申请的权限。",
    parseFailed: "无法读取该插件包中的权限。仍可导入，但请在导入后查看清单。",
    none: "该插件未声明额外权限。",
    unknownWarning: "未知权限——仅在信任插件作者时授予。",
    group: {
      slabApi: "Slab API",
      files: "文件访问",
      network: "网络",
      agent: "Agent",
      lsp: "语言服务器",
    },
    networkMode: {
      allowlist: "白名单",
      blocked: "已阻止",
    },
    severity: {
      low: "低风险",
      medium: "中风险",
      high: "高风险",
    },
    prompt: {
      title: "插件权限请求",
      description: "{{name}} 请求调用 {{method}} {{path}}，需要 {{permission}} 权限。",
      allow: "允许",
      deny: "拒绝",
    },
    management: {
      title: "已授权的插件权限",
      description: "你在运行时弹窗中允许的 Slab API 权限。撤销某项后，下次调用会再次询问。",
      empty: "尚未授权任何插件权限。",
      revoke: "撤销",
      revokeAll: "全部撤销",
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
    installFailed: "安装插件失败",
    invalidPlugin: "所选插件无效",
    unknownValidationError: "未知的插件校验错误",
    actionFailed: "无法更新 {{name}}",
    stopped: "已停止 {{name}}",
    enabled: "已启用 {{name}}",
    launched: "已启动 {{name}}",
    disabled: "已禁用 {{name}}",
    imported: "已导入 {{name}}",
    installed: "已安装 {{name}}",
    updated: "已更新 {{name}}",
    uninstalled: "已卸载 {{name}}",
    importCancelled: "插件包上传已取消",
  },
} as const;
