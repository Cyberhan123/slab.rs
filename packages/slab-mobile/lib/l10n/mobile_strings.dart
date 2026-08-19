/// Mobile-only chrome strings (connect screen, session list, chat composer).
///
/// Shared strings (approvals, common actions, runtime labels) come from the
/// generated `SlabCatalog` (same source as web/desktop). Keys below exist only
/// on mobile — they deliberately do NOT go into `packages/slab-i18n` because
/// its unused-keys guard would flag catalog entries no web consumer imports.
library;

const _strings = <String, Map<String, String>>{
  'en-US': {
    'mobile.connect.title': 'Connect to slab-server',
    'mobile.connect.baseUrl': 'Server URL',
    'mobile.connect.token': 'Access token (optional)',
    'mobile.connect.test': 'Test connection',
    'mobile.connect.testing': 'Testing…',
    'mobile.connect.ok': 'Connected — slab-server v{{version}}',
    'mobile.connect.unreachable': 'Could not reach the server',
    'mobile.connect.save': 'Save and continue',
    'mobile.connect.invalidUrl': 'Enter a valid http(s) URL',
    'mobile.sessions.title': 'Conversations',
    'mobile.sessions.empty': 'No conversations yet',
    'mobile.sessions.new': 'New conversation',
    'mobile.sessions.rename': 'Rename',
    'mobile.sessions.delete': 'Delete',
    'mobile.sessions.nameLabel': 'Name',
    'mobile.sessions.serverOffline': 'Server offline — retrying',
    'mobile.sessions.serverOnline': 'Connected',
    'mobile.setup.title': 'Server setup required',
    'mobile.setup.description':
        'This slab-server has not finished its one-time setup. Complete it on the desktop app (or the web shell), then this screen advances automatically.',
    'mobile.setup.checking': 'Checking server state…',
    'mobile.chat.inputHint': 'Message slab…',
    'mobile.chat.send': 'Send',
    'mobile.chat.stop': 'Stop',
    'mobile.chat.connecting': 'Connecting…',
    'mobile.chat.reconnecting': 'Reconnecting…',
    'mobile.chat.modelLoading': 'Loading model…',
    'mobile.chat.restoreFailed': 'Could not restore the conversation: {{message}}',
    'mobile.chat.denied': 'Denied',
    'mobile.chat.approve': 'Approve',
    'mobile.tool.running': 'Running',
    'mobile.tool.awaitingApproval': 'Awaiting approval',
    'mobile.tool.done': 'Done',
    'mobile.tool.failed': 'Failed',
    'mobile.tool.output': 'Output',
  },
  'zh-CN': {
    'mobile.connect.title': '连接 slab-server',
    'mobile.connect.baseUrl': '服务器地址',
    'mobile.connect.token': '访问令牌（可选）',
    'mobile.connect.test': '测试连接',
    'mobile.connect.testing': '测试中…',
    'mobile.connect.ok': '已连接 — slab-server v{{version}}',
    'mobile.connect.unreachable': '无法连接服务器',
    'mobile.connect.save': '保存并继续',
    'mobile.connect.invalidUrl': '请输入有效的 http(s) 地址',
    'mobile.sessions.title': '会话',
    'mobile.sessions.empty': '还没有会话',
    'mobile.sessions.new': '新会话',
    'mobile.sessions.rename': '重命名',
    'mobile.sessions.delete': '删除',
    'mobile.sessions.nameLabel': '名称',
    'mobile.sessions.serverOffline': '服务器离线 — 正在重试',
    'mobile.sessions.serverOnline': '已连接',
    'mobile.setup.title': '服务器需要初始化',
    'mobile.setup.description': '该 slab-server 尚未完成一次性初始化。请在桌面端（或 Web 壳）完成初始化，本页会自动进入。',
    'mobile.setup.checking': '正在检查服务器状态…',
    'mobile.chat.inputHint': '给 slab 发消息…',
    'mobile.chat.send': '发送',
    'mobile.chat.stop': '停止',
    'mobile.chat.connecting': '连接中…',
    'mobile.chat.reconnecting': '重连中…',
    'mobile.chat.modelLoading': '模型加载中…',
    'mobile.chat.restoreFailed': '恢复会话失败：{{message}}',
    'mobile.chat.denied': '拒绝',
    'mobile.chat.approve': '批准',
    'mobile.tool.running': '运行中',
    'mobile.tool.awaitingApproval': '等待批准',
    'mobile.tool.done': '完成',
    'mobile.tool.failed': '失败',
    'mobile.tool.output': '输出',
  },
};

/// Translate a mobile-only key for `locale`, interpolating `{{var}}`.
String mobileT(String locale, String key, [Map<String, String> args = const {}]) {
  var raw = _strings[locale]?[key] ?? _strings['en-US']?[key] ?? key;
  for (final entry in args.entries) {
    raw = raw.replaceAll('{{${entry.key}}}', entry.value);
  }
  return raw;
}
