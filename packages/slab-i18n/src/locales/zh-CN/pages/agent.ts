export const agent = {
  actions: {
    approve: '批准',
    interrupt: '中断',
    newThread: '新线程',
    reject: '拒绝',
  },
  composer: {
    placeholder: '让 Agent 检查、运行工具或修改工作区...',
    send: '发送消息',
  },
  connection: {
    connected: '事件已连接',
    idle: '无事件流',
    reconnecting: '重连中',
  },
  empty: {
    title: '启动 Agent 线程',
    description: '发送任务以运行带工具能力的 Slab Agent。',
  },
  header: {
    title: 'Agent',
    subtitle: '带工具能力的 Agent 工作流',
  },
  message: {
    agent: 'Agent',
    user: '你',
    waiting: '正在等待 Agent...',
  },
  modelPicker: {
    emptyLabel: '没有聊天模型',
    groupLabel: 'Agent 模型',
    placeholder: '选择模型',
  },
  status: {
    completed: '已完成',
    errored: '出错',
    idle: '空闲',
    pending: '等待中',
    running: '运行中',
    shutdown: '已中断',
  },
  timeline: {
    approvalRequired: '{{tool}} 需要批准',
    approved: '工具已批准',
    empty: 'Agent 事件会显示在这里。',
    interrupted: '线程已中断',
    lagged: '事件流跳过了较早消息',
    rejected: '工具已拒绝',
    title: 'Agent 事件',
    toolOutput: '工具输出',
    toolStarted: '{{tool}} 已启动',
    turnCompleted: '回合已完成',
    turnFailed: '回合失败',
  },
  toast: {
    approvalFailed: '发送批准失败',
    approvalNotDelivered: '批准未送达',
    interruptFailed: '中断线程失败',
    requestFailed: 'Agent 请求失败',
    selectModel: '请先选择 Agent 模型',
  },
} as const;
