export const assistant = {
  actions: {
    approve: 'Approve',
    reject: 'Reject',
  },
  approval: {
    title: 'Approval required',
    command: 'command',
    fileChange: 'file change',
    plan: 'plan',
    runOnce: 'Allow once',
    alwaysInWorkspace: 'Always in workspace',
    always: 'Always allow',
  },
  planMode: {
    exit: 'Exit',
  },
  header: {
    newSession: 'New session',
  },
  runtime: {
    newChat: 'New assistant',
    newConversation: 'New conversation',
    workspace: 'Workspace',
  },
  modelPicker: {
    groupLabel: 'Assistant model',
    emptyLabel: 'No assistant models',
  },
  greeting: {
    morning: 'Good morning',
    afternoon: 'Good afternoon',
    evening: 'Good evening',
  },
  hero: {
    description: 'How can I assist your creative workflow today?',
  },
  loading: {
    title: 'Loading this session...',
    description: 'Restoring the saved conversation history before you continue.',
  },
  history: {
    restored: 'History restored',
  },
  compaction: {
    autoCompacting: 'Compacting context…',
    autoCompacted: 'Context compacted',
    manuallyCompacting: 'Compacting context…',
    manuallyCompacted: 'Context compacted',
  },
  modelLoad: {
    downloading: 'Downloading model...',
    loading: 'Loading model...',
  },
  usage: {
    prompt: 'In {{formatted}}',
    completion: 'Out {{formatted}}',
    cached: 'Cached {{formatted}}',
    used: 'Used {{percent}}%',
    total: '{{formatted}} tokens',
  },
  status: {
    preparingSession: 'Preparing session',
    loadingSessionHistory: 'Loading session history',
    creatingSession: 'Creating session',
    deletingSession: 'Deleting session',
    loadingModels: 'Loading models',
    downloading: 'Downloading',
    needsDownload: 'Needs download',
    preparing: 'Preparing',
    cloudModel: 'Cloud model',
    contextWindow: '{{formatted}} context',
    runtimeContextWindow: '{{formatted}} runtime context',
  },
  composer: {
    placeholder: 'Type a message or drop files...',
    commandSkill: 'Skill',
    commandMcp: 'MCP',
    permission: {
      title: 'Permission mode',
      requestApproval: 'Request approval',
      approveForMe: 'Approve for me',
      fullControl: 'Full control',
      custom: 'Custom',
    },
    interaction: {
      plan: 'Plan',
    },
    stopGeneratingResponse: 'Stop generating response',
    sendMessage: 'Send message',
    deepThink: 'Deep think',
    reasoningEffort: 'Reasoning',
    reasoning: {
      low: 'Low',
      medium: 'Medium',
      high: 'High',
    },
  },
  sessionSheet: {
    title: 'Manage sessions',
    description: 'Switch and clean up conversations without leaving the assistant stage.',
    current: 'Current',
    live: 'Live',
    delete: 'Delete',
  },
  message: {
    assistant: 'Assistant',
    user: 'User',
    rollback: 'Rollback',
    confirmRollback: 'Retract this message and everything after it?',
    cancelEdit: 'Cancel edit',
  },
  thinking: {
    loading: 'Thinking...',
    thoughtForAFewSeconds: 'Thought for a few seconds',
    thoughtForSeconds: 'Thought for {{seconds}} seconds',
  },
  dialog: {
    title: 'Switch model for this conversation?',
    description:
      'Choose whether the new model should keep using this session history or start from a clean session.',
    switchingSummary:
      'You are switching from <strong>{{from}}</strong> to <strong>{{to}}</strong>.',
    sessionSummary_one: '<strong>{{label}}</strong> already has {{count}} message.',
    sessionSummary_other: '<strong>{{label}}</strong> already has {{count}} messages.',
    keepTitle: 'Keep current session',
    keepDescription:
      'The new model will continue from this conversation and see the existing message history.',
    createTitle: 'Create new session',
    createDescription:
      'Start with a clean session and keep the previous conversation attached to the old model.',
  },
  sessionSummary: {
    currentSession: 'Current session',
  },
  toast: {
    waitBeforeDeletingSessions: 'Wait for the current response to finish before deleting sessions.',
    sessionSyncing: 'Assistant session is still syncing. Please try again in a moment.',
    modelLoadRetry: 'Model load failed, re-downloading and retrying once...',
    failedToPrepareModel: 'Failed to prepare assistant model.',
    waitBeforeSwitchingModels:
      'Wait for the current response or session sync to finish before switching models.',
    failedToLoadSession: 'Failed to load assistant session.',
    failedToCreateSession: 'Failed to create assistant session.',
    failedToUpdateSession: 'Failed to update assistant session.',
    failedToDeleteSession: 'Failed to delete assistant session.',
    compactFailed: 'Failed to compact the conversation.',
    forkFailed: 'Failed to fork the conversation.',
  },
  connection: {
    connected: 'Events connected',
  },
  error: {
    selectModelFirst: 'Please select an assistant model first.',
    selectedModelUnavailable: 'Selected model is not available.',
  },
} as const;
