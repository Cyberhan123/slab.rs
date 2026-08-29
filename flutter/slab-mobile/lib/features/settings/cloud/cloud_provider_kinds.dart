/// Presentation metadata for the cloud-provider family dropdown. Port of the
/// desktop `cloud-provider-kinds.ts` — `value` mirrors `ProviderFamily::all_str()`
/// (snake_case), which mirrors genai's `AdapterKind`. `defaultApiBase` /
/// `defaultKeyEnv` are UX hints only.
library;

enum CloudProviderGroup { popular, china, gateways, local, other }

class CloudProviderKind {
  const CloudProviderKind({
    required this.value,
    required this.label,
    required this.group,
    required this.defaultApiBase,
    required this.defaultKeyEnv,
  });

  final String value;
  final String label;
  final CloudProviderGroup group;
  final String defaultApiBase;
  final String defaultKeyEnv;
}

/// The "Other / custom OpenAI-compatible endpoint" family.
const openAiCompatibleValue = 'openai_compatible';

const cloudProviderKinds = [
  // ── Other / custom ───────────────────────────────────────────────────────
  CloudProviderKind(value: openAiCompatibleValue, label: 'Other (OpenAI-compatible)', group: CloudProviderGroup.other, defaultApiBase: '', defaultKeyEnv: ''),
  // ── Popular ──────────────────────────────────────────────────────────────
  CloudProviderKind(value: 'openai', label: 'OpenAI', group: CloudProviderGroup.popular, defaultApiBase: 'https://api.openai.com/v1', defaultKeyEnv: 'OPENAI_API_KEY'),
  CloudProviderKind(value: 'openai_resp', label: 'OpenAI Responses', group: CloudProviderGroup.popular, defaultApiBase: 'https://api.openai.com/v1', defaultKeyEnv: 'OPENAI_API_KEY'),
  CloudProviderKind(value: 'anthropic', label: 'Anthropic', group: CloudProviderGroup.popular, defaultApiBase: 'https://api.anthropic.com/v1', defaultKeyEnv: 'ANTHROPIC_API_KEY'),
  CloudProviderKind(value: 'gemini', label: 'Google Gemini', group: CloudProviderGroup.popular, defaultApiBase: 'https://generativelanguage.googleapis.com/v1beta', defaultKeyEnv: 'GEMINI_API_KEY'),
  CloudProviderKind(value: 'groq', label: 'Groq', group: CloudProviderGroup.popular, defaultApiBase: 'https://api.groq.com/openai/v1', defaultKeyEnv: 'GROQ_API_KEY'),
  CloudProviderKind(value: 'deep_seek', label: 'DeepSeek', group: CloudProviderGroup.popular, defaultApiBase: 'https://api.deepseek.com/v1', defaultKeyEnv: 'DEEPSEEK_API_KEY'),
  CloudProviderKind(value: 'xai', label: 'xAI (Grok)', group: CloudProviderGroup.popular, defaultApiBase: 'https://api.x.ai/v1', defaultKeyEnv: 'XAI_API_KEY'),
  CloudProviderKind(value: 'cohere', label: 'Cohere', group: CloudProviderGroup.popular, defaultApiBase: 'https://api.cohere.com/v1', defaultKeyEnv: 'COHERE_API_KEY'),
  // ── China ────────────────────────────────────────────────────────────────
  CloudProviderKind(value: 'zai', label: 'Z.AI (GLM)', group: CloudProviderGroup.china, defaultApiBase: 'https://api.z.ai/api/paas/v4', defaultKeyEnv: 'ZAI_API_KEY'),
  CloudProviderKind(value: 'big_model', label: 'BigModel (GLM)', group: CloudProviderGroup.china, defaultApiBase: '', defaultKeyEnv: ''),
  CloudProviderKind(value: 'moonshot', label: 'Moonshot (Kimi)', group: CloudProviderGroup.china, defaultApiBase: 'https://api.moonshot.cn/v1', defaultKeyEnv: 'MOONSHOT_API_KEY'),
  CloudProviderKind(value: 'aliyun', label: 'Aliyun (DashScope)', group: CloudProviderGroup.china, defaultApiBase: 'https://dashscope.aliyuncs.com/compatible-mode/v1', defaultKeyEnv: 'ALIYUN_API_KEY'),
  CloudProviderKind(value: 'baidu', label: 'Baidu (ERNIE)', group: CloudProviderGroup.china, defaultApiBase: '', defaultKeyEnv: ''),
  CloudProviderKind(value: 'mimo', label: 'Mimo', group: CloudProviderGroup.china, defaultApiBase: '', defaultKeyEnv: ''),
  CloudProviderKind(value: 'aihubmix', label: 'AIHubMix', group: CloudProviderGroup.china, defaultApiBase: 'https://aihubmix.com/v1', defaultKeyEnv: 'AIHUBMIX_API_KEY'),
  CloudProviderKind(value: 'mini_max', label: 'MiniMax', group: CloudProviderGroup.china, defaultApiBase: '', defaultKeyEnv: 'MINIMAX_API_KEY'),
  // ── Gateways & clouds ────────────────────────────────────────────────────
  CloudProviderKind(value: 'open_router', label: 'OpenRouter', group: CloudProviderGroup.gateways, defaultApiBase: 'https://openrouter.ai/api/v1', defaultKeyEnv: 'OPEN_ROUTER_API_KEY'),
  CloudProviderKind(value: 'together', label: 'Together AI', group: CloudProviderGroup.gateways, defaultApiBase: 'https://api.together.xyz/v1', defaultKeyEnv: 'TOGETHER_API_KEY'),
  CloudProviderKind(value: 'fireworks', label: 'Fireworks AI', group: CloudProviderGroup.gateways, defaultApiBase: 'https://api.fireworks.ai/inference/v1', defaultKeyEnv: 'FIREWORKS_API_KEY'),
  CloudProviderKind(value: 'nebius', label: 'Nebius', group: CloudProviderGroup.gateways, defaultApiBase: '', defaultKeyEnv: ''),
  CloudProviderKind(value: 'github_copilot', label: 'GitHub Copilot Models', group: CloudProviderGroup.gateways, defaultApiBase: '', defaultKeyEnv: ''),
  CloudProviderKind(value: 'vertex', label: 'Google Vertex AI', group: CloudProviderGroup.gateways, defaultApiBase: '', defaultKeyEnv: ''),
  CloudProviderKind(value: 'bedrock_api', label: 'AWS Bedrock', group: CloudProviderGroup.gateways, defaultApiBase: '', defaultKeyEnv: 'BEDROCK_API_KEY'),
  CloudProviderKind(value: 'open_code_go', label: 'OpenCode Go', group: CloudProviderGroup.gateways, defaultApiBase: '', defaultKeyEnv: ''),
  // ── Local ────────────────────────────────────────────────────────────────
  CloudProviderKind(value: 'ollama', label: 'Ollama', group: CloudProviderGroup.local, defaultApiBase: 'http://localhost:11434', defaultKeyEnv: ''),
  CloudProviderKind(value: 'ollama_cloud', label: 'Ollama Cloud', group: CloudProviderGroup.local, defaultApiBase: '', defaultKeyEnv: ''),
];

const _groupOrder = [
  CloudProviderGroup.popular,
  CloudProviderGroup.china,
  CloudProviderGroup.gateways,
  CloudProviderGroup.local,
  CloudProviderGroup.other,
];

const _groupLabels = {
  CloudProviderGroup.popular: 'Popular',
  CloudProviderGroup.china: 'China',
  CloudProviderGroup.gateways: 'Gateways & clouds',
  CloudProviderGroup.local: 'Local',
  CloudProviderGroup.other: 'Other',
};

/// Kind metadata for a family value, falling back to "Other" for unknowns.
CloudProviderKind kindForFamily(String family) =>
    cloudProviderKinds.where((kind) => kind.value == family).firstOrNull ?? openAiCompatibleKind;

CloudProviderKind get openAiCompatibleKind =>
    cloudProviderKinds.firstWhere((kind) => kind.value == openAiCompatibleValue);

/// Kinds grouped in display order (label included, for the grouped picker).
List<(String, List<CloudProviderKind>)> kindsByGroup() => [
      for (final group in _groupOrder)
        if (cloudProviderKinds.any((kind) => kind.group == group))
          (_groupLabels[group]!, cloudProviderKinds.where((kind) => kind.group == group).toList(growable: false)),
    ];
