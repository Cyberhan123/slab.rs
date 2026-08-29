/// Session display-label helpers: the default-label family (both locales +
/// the legacy set) and the first-prompt title truncation. Port of the
/// zustand assistant-store label logic (`use-assistant-conversation-list`).
library;

/// Server-side default names for a fresh assistant session (both locales)
/// plus the legacy chat names — a session still carrying one of these gets
/// retitled from its first user prompt.
const defaultAssistantLabels = {
  'New assistant', // en newChat
  'New conversation', // en newConversation
  '新助手', // zh newChat
  '新会话', // zh newConversation
  // Legacy chat-era defaults (pre-assistant sessions).
  'New Conversation',
  '新对话',
};

bool isDefaultSessionLabel(String? label) =>
    label == null || label.isEmpty || defaultAssistantLabels.contains(label.trim());

/// First-prompt title: trimmed, capped at 42 chars + ellipsis.
String createConversationLabel(String prompt, String fallback) {
  final trimmed = prompt.trim();
  if (trimmed.isEmpty) return fallback;
  return trimmed.length > 42 ? '${trimmed.substring(0, 42)}...' : trimmed;
}
