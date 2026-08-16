/**
 * Presentation metadata for the cloud-provider family dropdown.
 *
 * `value` strings mirror `ProviderFamily::all_str()` in `crates/slab-config` (snake_case), which
 * in turn mirror genai's `AdapterKind` variants. When genai adds an `AdapterKind`, add it here AND
 * to the Rust enum + `slab-cloud-provider`'s `family_to_adapter_kind`.
 *
 * `defaultApiBase` / `defaultKeyEnv` are UX hints only — at runtime credential resolution uses
 * genai's `AdapterKind::default_key_env_name()` and the values stored in settings.
 */

export type CloudProviderGroup = 'popular' | 'china' | 'gateways' | 'local' | 'other';

export interface CloudProviderKind {
  value: string;
  label: string;
  group: CloudProviderGroup;
  defaultApiBase: string;
  defaultKeyEnv: string;
}

/** The "Other / custom OpenAI-compatible endpoint" family. */
export const OPENAI_COMPATIBLE_VALUE = 'openai_compatible';

export const CLOUD_PROVIDER_KINDS: CloudProviderKind[] = [
  // ── Other / custom ───────────────────────────────────────────────────────
  { value: OPENAI_COMPATIBLE_VALUE, label: 'Other (OpenAI-compatible)', group: 'other', defaultApiBase: '', defaultKeyEnv: '' },

  // ── Popular ───────────────────────────────────────────────────────────────
  { value: 'openai', label: 'OpenAI', group: 'popular', defaultApiBase: 'https://api.openai.com/v1', defaultKeyEnv: 'OPENAI_API_KEY' },
  { value: 'openai_resp', label: 'OpenAI Responses', group: 'popular', defaultApiBase: 'https://api.openai.com/v1', defaultKeyEnv: 'OPENAI_API_KEY' },
  { value: 'anthropic', label: 'Anthropic', group: 'popular', defaultApiBase: 'https://api.anthropic.com/v1', defaultKeyEnv: 'ANTHROPIC_API_KEY' },
  { value: 'gemini', label: 'Google Gemini', group: 'popular', defaultApiBase: 'https://generativelanguage.googleapis.com/v1beta', defaultKeyEnv: 'GEMINI_API_KEY' },
  { value: 'groq', label: 'Groq', group: 'popular', defaultApiBase: 'https://api.groq.com/openai/v1', defaultKeyEnv: 'GROQ_API_KEY' },
  { value: 'deep_seek', label: 'DeepSeek', group: 'popular', defaultApiBase: 'https://api.deepseek.com/v1', defaultKeyEnv: 'DEEPSEEK_API_KEY' },
  { value: 'xai', label: 'xAI (Grok)', group: 'popular', defaultApiBase: 'https://api.x.ai/v1', defaultKeyEnv: 'XAI_API_KEY' },
  { value: 'cohere', label: 'Cohere', group: 'popular', defaultApiBase: 'https://api.cohere.com/v1', defaultKeyEnv: 'COHERE_API_KEY' },

  // ── China ─────────────────────────────────────────────────────────────────
  { value: 'zai', label: 'Z.AI (GLM)', group: 'china', defaultApiBase: 'https://api.z.ai/api/paas/v4', defaultKeyEnv: 'ZAI_API_KEY' },
  { value: 'big_model', label: 'BigModel (GLM)', group: 'china', defaultApiBase: '', defaultKeyEnv: '' },
  { value: 'moonshot', label: 'Moonshot (Kimi)', group: 'china', defaultApiBase: 'https://api.moonshot.cn/v1', defaultKeyEnv: 'MOONSHOT_API_KEY' },
  { value: 'aliyun', label: 'Aliyun (DashScope)', group: 'china', defaultApiBase: 'https://dashscope.aliyuncs.com/compatible-mode/v1', defaultKeyEnv: 'ALIYUN_API_KEY' },
  { value: 'baidu', label: 'Baidu (ERNIE)', group: 'china', defaultApiBase: '', defaultKeyEnv: '' },
  { value: 'mimo', label: 'Mimo', group: 'china', defaultApiBase: '', defaultKeyEnv: '' },
  { value: 'aihubmix', label: 'AIHubMix', group: 'china', defaultApiBase: 'https://aihubmix.com/v1', defaultKeyEnv: 'AIHUBMIX_API_KEY' },
  { value: 'mini_max', label: 'MiniMax', group: 'china', defaultApiBase: '', defaultKeyEnv: 'MINIMAX_API_KEY' },

  // ── Gateways & clouds ─────────────────────────────────────────────────────
  { value: 'open_router', label: 'OpenRouter', group: 'gateways', defaultApiBase: 'https://openrouter.ai/api/v1', defaultKeyEnv: 'OPEN_ROUTER_API_KEY' },
  { value: 'together', label: 'Together AI', group: 'gateways', defaultApiBase: 'https://api.together.xyz/v1', defaultKeyEnv: 'TOGETHER_API_KEY' },
  { value: 'fireworks', label: 'Fireworks AI', group: 'gateways', defaultApiBase: 'https://api.fireworks.ai/inference/v1', defaultKeyEnv: 'FIREWORKS_API_KEY' },
  { value: 'nebius', label: 'Nebius', group: 'gateways', defaultApiBase: '', defaultKeyEnv: '' },
  { value: 'github_copilot', label: 'GitHub Copilot Models', group: 'gateways', defaultApiBase: '', defaultKeyEnv: '' },
  { value: 'vertex', label: 'Google Vertex AI', group: 'gateways', defaultApiBase: '', defaultKeyEnv: '' },
  { value: 'bedrock_api', label: 'AWS Bedrock', group: 'gateways', defaultApiBase: '', defaultKeyEnv: 'BEDROCK_API_KEY' },
  { value: 'open_code_go', label: 'OpenCode Go', group: 'gateways', defaultApiBase: '', defaultKeyEnv: '' },

  // ── Local ─────────────────────────────────────────────────────────────────
  { value: 'ollama', label: 'Ollama', group: 'local', defaultApiBase: 'http://localhost:11434', defaultKeyEnv: '' },
  { value: 'ollama_cloud', label: 'Ollama Cloud', group: 'local', defaultApiBase: '', defaultKeyEnv: '' },
];

const GROUP_ORDER: CloudProviderGroup[] = ['popular', 'china', 'gateways', 'local', 'other'];

/** Kind metadata for a family value, falling back to the "Other" kind for unknown values. */
export function kindForFamily(family: string): CloudProviderKind {
  return (
    CLOUD_PROVIDER_KINDS.find((kind) => kind.value === family) ??
    CLOUD_PROVIDER_KINDS.find((kind) => kind.value === OPENAI_COMPATIBLE_VALUE)!
  );
}

/** Kinds grouped in display order, for the grouped `<Select>` dropdown. */
export function kindsByGroup(): { group: CloudProviderGroup; kinds: CloudProviderKind[] }[] {
  return GROUP_ORDER.map((group) => ({
    group,
    kinds: CLOUD_PROVIDER_KINDS.filter((kind) => kind.group === group),
  })).filter((entry) => entry.kinds.length > 0);
}
