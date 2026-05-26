export const agent = {
  actions: {
    approve: 'Approve',
    interrupt: 'Interrupt',
    newThread: 'New thread',
    reject: 'Reject',
  },
  composer: {
    placeholder: 'Ask the agent to inspect, run tools, or change the workspace...',
    send: 'Send message',
  },
  connection: {
    connected: 'Events connected',
    idle: 'No stream',
    reconnecting: 'Reconnecting',
  },
  empty: {
    title: 'Start an agent thread',
    description: 'Send a task to run the tool-enabled Slab agent.',
  },
  header: {
    title: 'Agent',
    subtitle: 'Tool-enabled agent workflow',
  },
  message: {
    agent: 'Agent',
    user: 'You',
    waiting: 'Waiting for the agent...',
  },
  modelPicker: {
    emptyLabel: 'No chat models',
    groupLabel: 'Agent model',
    placeholder: 'Select model',
  },
  status: {
    completed: 'Completed',
    errored: 'Errored',
    idle: 'Idle',
    pending: 'Pending',
    running: 'Running',
    shutdown: 'Interrupted',
  },
  timeline: {
    approvalRequired: '{{tool}} needs approval',
    approved: 'Tool approved',
    empty: 'Agent events will appear here.',
    interrupted: 'Thread interrupted',
    lagged: 'Event stream skipped older messages',
    rejected: 'Tool rejected',
    title: 'Agent Events',
    toolOutput: 'Tool output',
    toolStarted: '{{tool}} started',
    turnCompleted: 'Turn completed',
    turnFailed: 'Turn failed',
  },
  toast: {
    approvalFailed: 'Failed to send approval',
    approvalNotDelivered: 'Approval was not delivered',
    interruptFailed: 'Failed to interrupt thread',
    requestFailed: 'Agent request failed',
    selectModel: 'Select an agent model first',
  },
} as const;
