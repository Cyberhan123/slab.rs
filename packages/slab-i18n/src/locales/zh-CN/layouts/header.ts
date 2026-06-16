export const header = {
  select: {
    loadingOptions: '正在加载选项...',
    selectOption: '选择选项',
    options: '选项',
    noOptions: '暂无可用选项',
  },
  search: {
    chat: '搜索任务...',
    default: '搜索页面、工具或设置...',
  },
  context: {
    activeWorkspace: '当前工作区',
    desktop: 'Slab 桌面端',
  },
  windowControls: {
    toolbar: '窗口控制',
    minimize: '最小化窗口',
    toggleMaximize: '最大化窗口',
    close: '关闭窗口',
    errors: {
      minimize: '最小化窗口失败。',
      toggleMaximize: '最大化窗口失败。',
      close: '关闭窗口失败。',
      capabilityRestart: '窗口控制权限变更后需要重启 Tauri。',
    },
  },
} as const;
