export const workspace = {
  header: {
    title: 'Workspace',
    subtitle: 'Folder mode',
  },
  actions: {
    openFolder: 'Open folder',
    closeWorkspace: 'Close',
    reopen: 'Open',
  },
  empty: {
    title: 'No workspace open',
    description: 'Open a folder or return to a recent workspace.',
  },
  recent: {
    title: 'Recent workspaces',
    empty: 'No recent workspaces',
  },
  tree: {
    title: 'Files',
    loading: 'Loading files',
    truncated: 'Directory limit reached',
  },
  editor: {
    emptyTitle: 'No file selected',
    emptyDescription: 'Select a source file from the tree.',
    tooLarge: 'Preview unavailable',
  },
  tabs: {
    close: 'Close {{name}}',
  },
  plugins: {
    title: 'Workspace plugins',
    empty: 'No plugins available',
    enable: 'Enable in workspace',
  },
  toast: {
    openFailed: 'Failed to open workspace',
    closeFailed: 'Failed to close workspace',
    fileFailed: 'Failed to open file',
    pluginFailed: 'Failed to update plugin preference',
  },
} as const;
