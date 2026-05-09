export const workspace = {
  header: {
    title: '工作区',
    subtitle: '文件夹模式',
  },
  actions: {
    openFolder: '打开文件夹',
    closeWorkspace: '关闭',
    reopen: '打开',
  },
  empty: {
    title: '未打开工作区',
    description: '打开文件夹或返回最近工作区。',
  },
  recent: {
    title: '最近工作区',
    empty: '暂无最近工作区',
  },
  tree: {
    title: '文件',
    loading: '正在加载文件',
    truncated: '目录已达到显示上限',
  },
  editor: {
    emptyTitle: '未选择文件',
    emptyDescription: '从文件树中选择源文件。',
    tooLarge: '无法预览',
  },
  tabs: {
    close: '关闭 {{name}}',
  },
  plugins: {
    title: '工作区插件',
    empty: '无可用插件',
    enable: '在工作区启用',
  },
  toast: {
    openFailed: '打开工作区失败',
    closeFailed: '关闭工作区失败',
    fileFailed: '打开文件失败',
    pluginFailed: '更新插件偏好失败',
  },
} as const;
