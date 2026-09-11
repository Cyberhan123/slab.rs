export const agentRollouts = {
  disabled: {
    title: 'Agent debug tracing is off',
    description:
      'Enable "Agent debug tracing" in Settings to inspect rollouts, per-turn prompts, and trace bundles.',
  },
  rail: {
    title: 'Sessions',
    empty: 'No rollout sessions found',
    traceBadge: 'trace',
  },
  tabs: {
    timeline: 'Timeline',
    lines: 'Raw lines',
    trace: 'Trace',
  },
  timeline: {
    empty: 'No items in this rollout',
    turnPrompt: 'Turn {{index}} prompt',
    turnPromptMissing: 'no TurnState record',
    turnPromptHint:
      'The exact input messages the model was sent for this turn (system fragments, injected context, full conversation).',
  },
  lines: {
    empty: 'No rollout lines',
    filter: 'Filter by rollout type',
    all: 'All',
    total: '{{shown}} of {{total}} lines',
    loadMore: 'Load more',
    index: '#{{index}}',
  },
  trace: {
    missing:
      'No trace bundle for this thread. Bundles are written while "Agent debug tracing" is enabled.',
    manifest: 'Manifest',
    conversation: 'Conversation the model saw',
    conversationEmpty: 'The reducer produced no conversation for this bundle',
    events: 'Trace events',
    eventsCount: '{{shown}} of {{total}} events',
    loadMore: 'Load more',
  },
} as const;
