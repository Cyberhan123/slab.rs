export const chat = {
  header: {
    title: 'Assistant',
    subtitle: 'Talk with AI assistants',
  },
  runtime: {
    requestAborted: 'Request aborted',
    requestFailed: 'Request failed, please try again!',
    noData: 'No data',
    newChat: 'New chat',
    newConversation: 'New conversation',
    workspace: 'Workspace',
  },
  modelPicker: {
    groupLabel: 'Chat models',
    placeholder: 'Select model',
    emptyLabel: 'No chat models',
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
  emptyState: {
    title: 'Start a new thread and keep the stage focused.',
    description:
      'Ask for debugging help, refine a draft, or pass the current idea into image generation when it needs a visual direction.',
  },
  sessionSummary: {
    currentSession: 'Current session',
    nextSession: 'Next session',
    messageCount_one: '{{count}} message',
    messageCount_other: '{{count}} messages',
  },
  status: {
    preparingSession: 'Preparing session',
    loadingSessionHistory: 'Loading session history',
    creatingSession: 'Creating session',
    deletingSession: 'Deleting session',
    loadingModels: 'Loading models',
    selectModel: 'Select model',
    downloading: 'Downloading',
    needsDownload: 'Needs download',
    preparing: 'Preparing',
    cloudModel: 'Cloud model',
    contextWindow: '{{formatted}} context',
  },
  composer: {
    placeholder: 'Type a message or drop files...',
    generateImage: 'Generate image',
    webSearch: 'Web search',
    voiceCapture: 'Voice capture',
    stopGeneratingResponse: 'Stop generating response',
    sendMessage: 'Send message',
    deepThink: 'Deep think',
    deepThinkOn: 'Deep think on',
    deepThinkUnavailable: 'Deep think unavailable',
  },
  sessionSheet: {
    title: 'Manage sessions',
    description: 'Switch and clean up conversations without leaving the chat stage.',
    current: 'Current',
    live: 'Live',
    open: 'Open',
    delete: 'Delete',
  },
  message: {
    assistant: 'Assistant',
    user: 'User',
    waitingForResponse: 'Waiting for response...',
    copy: 'Copy',
    continue: 'Continue',
    retry: 'Retry',
  },
  thinking: {
    loading: 'Thinking...',
    ready: 'Reasoning trace',
    empty: 'Waiting for reasoning content...',
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
    cancel: 'Cancel',
  },
  summaryCard: {
    latestSession: 'Latest session',
    createSession: 'Create session',
    manageSessions: 'Manage sessions',
  },
  toast: {
    waitForCurrentResponse: 'Wait for the current response to finish before changing sessions.',
    currentSessionAlreadyEmpty: 'The current session is already empty.',
    waitBeforeDeletingSessions: 'Wait for the current response to finish before deleting sessions.',
    sessionSyncing: 'Chat session is still syncing. Please try again in a moment.',
    downloaded: 'Downloaded {{model}}',
    modelLoadRetry: 'Model load failed, re-downloading and retrying once...',
    failedToPrepareModel: 'Failed to prepare chat model.',
    waitBeforeSwitchingModels:
      'Wait for the current response or session sync to finish before switching models.',
    failedToCreateSession: 'Failed to create chat session.',
    failedToDeleteSession: 'Failed to delete chat session.',
    unknownError: 'Unknown error',
  },
  error: {
    selectModelFirst: 'Please select a chat model first.',
    selectedModelUnavailable: 'Selected model is not available.',
    selectedModelMissing: 'Selected model does not exist in the catalog.',
    selectedModelNotLocal: 'Selected model is not a local chat model.',
    downloadTimedOut: 'Model download timed out.',
  },
} as const;
