export const header = {
  select: {
    loadingOptions: 'Loading options...',
    selectOption: 'Select option',
    options: 'Options',
    noOptions: 'No options available',
  },
  search: {
    chat: 'Search tasks...',
    default: 'Search pages, tools, or settings...',
  },
  context: {
    activeWorkspace: 'Active Workspace',
    desktop: 'Slab Desktop',
  },
  windowControls: {
    toolbar: 'Window controls',
    minimize: 'Minimize window',
    toggleMaximize: 'Maximize window',
    close: 'Close window',
    errors: {
      minimize: 'Failed to minimize the window.',
      toggleMaximize: 'Failed to maximize the window.',
      close: 'Failed to close the window.',
      capabilityRestart: 'Window controls need a Tauri restart after capability changes.',
    },
  },
} as const;
