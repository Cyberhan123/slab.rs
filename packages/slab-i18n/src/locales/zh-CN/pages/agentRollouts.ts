export const agentRollouts = {
  disabled: {
    title: 'Agent 调试追踪未开启',
    description: '在设置中开启「Agent 调试追踪」后才能检视 rollout、每轮完整提示词与 trace bundle。',
  },
  noThread: '此会话还没有 rollout 记录。发送第一条消息后即可在这里查看完整 rollout。',
  threadLabel: '线程',
  traceBadge: 'trace',
  tabs: {
    timeline: '时间线',
    lines: '原始行',
    trace: 'Trace',
  },
  timeline: {
    empty: '此 rollout 无条目',
    turnPrompt: '第 {{index}} 轮提示词',
    turnPromptMissing: '无 TurnState 记录',
    turnPromptHint: '该轮发给模型的完整输入消息(系统片段、注入上下文、完整会话)。',
  },
  lines: {
    empty: '无 rollout 行',
    filter: '按 rollout 类型过滤',
    search: '搜索行内容…',
    all: '全部',
    total: '共 {{total}} 行,已显示 {{shown}} 行',
    loadMore: '加载更多',
    index: '#{{index}}',
  },
  trace: {
    missing: '此线程没有 trace bundle。开启「Agent 调试追踪」后会写入 bundle。',
    manifest: '清单',
    conversation: '模型实际看到的会话',
    conversationEmpty: 'reducer 未从该 bundle 还原出会话',
    events: 'Trace 事件',
    eventsCount: '共 {{total}} 个事件,已显示 {{shown}} 个',
    loadMore: '加载更多',
  },
} as const;
