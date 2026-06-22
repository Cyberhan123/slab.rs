import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import SettingsPage from '@/pages/settings';
import i18n from '@slab/i18n';
import type { SettingsDocumentResponse } from '@/pages/settings/types';
import { renderDesktopScene } from '../test-utils';

const {
  mockMutateAsync,
  mockRefetch,
  mockSettingsData,
} = vi.hoisted(() => ({
  mockMutateAsync: vi.fn<() => Promise<Record<string, never>>>().mockResolvedValue({}),
  mockRefetch: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockSettingsData: {
    schema_version: 2,
    settings_path: 'C:/Slab/settings.json',
    warnings: [],
    sections: [
      {
        description_md: 'Runtime preferences used by the desktop shell.',
        id: 'runtime',
        subsections: [
          {
            description_md: 'Overrides specific to the GGML llama worker.',
            id: 'llama',
            properties: [
              {
                description_md: 'Override the llama context window length in tokens.',
                editable: true,
                effective_value: 4096,
                is_overridden: true,
                label: 'Context Length',
                override_value: 4096,
                pmid: 'runtime.ggml.backends.llama.context_length',
                schema: {
                  default_value: null,
                  minimum: 0,
                  type: 'integer',
                },
                search_terms: [],
              },
            ],
            title: 'Llama',
          },
        ],
        title: 'Runtime',
      },
    ],
  } as SettingsDocumentResponse,
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderSearch: vi.fn<() => void>(),
}));

vi.mock('@slab/api', async () => {
  const { createSlabApiMock } = await import('../support/mock-slab-api');

  return createSlabApiMock({
    defaultExport: {
      useMutation: vi.fn<() => unknown>(() => ({
        isPending: false,
        mutateAsync: mockMutateAsync,
      })),
      useQuery: vi.fn<() => unknown>(() => ({
        data: mockSettingsData,
        error: null,
        isLoading: false,
        refetch: mockRefetch,
      })),
    },
  });
});

describe('settings core flow e2e', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    mockSettingsData.warnings = [];
    mockSettingsData.sections = defaultSettingsSections();
    mockSettingsData.settings_path = 'C:/Slab/settings.json';
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('auto-saves edited settings and resets an override through the API contract', async () => {
    await renderDesktopScene(<SettingsPage />, { route: '/settings' });

    const contextLength = page.getByTestId('settings-input-runtime.ggml.backends.llama.context_length');
    await contextLength.fill('8192');
    await vi.advanceTimersByTimeAsync(700);

    await vi.waitFor(() => {
      expect(mockMutateAsync).toHaveBeenCalledWith({
        body: {
          op: 'set',
          value: 8192,
        },
        params: {
          path: {
            pmid: 'runtime.ggml.backends.llama.context_length',
          },
        },
      });
    });

    mockMutateAsync.mockClear();
    await page.getByTestId('settings-reset-runtime.ggml.backends.llama.context_length').click();

    await vi.waitFor(() => {
      expect(mockMutateAsync).toHaveBeenCalledWith({
        body: {
          op: 'unset',
        },
        params: {
          path: {
            pmid: 'runtime.ggml.backends.llama.context_length',
          },
        },
      });
    });
  });

  it('renders localized settings metadata and nested editors in zh-CN', async () => {
    await i18n.changeLanguage('zh-CN');
    mockSettingsData.settings_path = '/tmp/slab/settings.json';
    mockSettingsData.sections = localizedSettingsSections();

    await renderDesktopScene(<SettingsPage />, { route: '/settings' });

    await expect.element(page.getByRole('heading', { name: '运行时' })).toBeVisible();
    await expect.element(page.getByRole('heading', { name: '上下文长度' })).toBeVisible();
    await expect.element(page.getByRole('heading', { name: '提供商注册表' })).toBeVisible();
    await expect.element(page.getByRole('button', { name: '添加提供商' })).toBeVisible();
    await expect.element(page.getByText('暂无配置的提供商。')).toBeVisible();
    await expect.element(page.getByText('服务器名称')).toBeVisible();
    await expect.element(page.getByText('命令')).toBeVisible();
    await expect.element(page.getByText('已启用')).toBeVisible();

    const contextLength = page.getByTestId('settings-input-runtime.ggml.backends.llama.context_length');
    await contextLength.fill('8192ms');
    await vi.advanceTimersByTimeAsync(700);

    await expect.element(page.getByText('值必须是整数。')).toBeVisible();

    mockMutateAsync.mockClear();
    await page.getByTestId('settings-reset-runtime.ggml.backends.llama.context_length').click();

    await vi.waitFor(() => {
      expect(mockMutateAsync).toHaveBeenCalledWith({
        body: {
          op: 'unset',
        },
        params: {
          path: {
            pmid: 'runtime.ggml.backends.llama.context_length',
          },
        },
      });
    });
    await expect.element(page.getByText('已将 上下文长度 恢复为默认值。')).toBeVisible();
  });
});

function defaultSettingsSections(): SettingsDocumentResponse['sections'] {
  return [
    {
      description_md: 'Runtime preferences used by the desktop shell.',
      id: 'runtime',
      subsections: [
        {
          description_md: 'Overrides specific to the GGML llama worker.',
          id: 'llama',
          properties: [
            {
              description_md: 'Override the llama context window length in tokens.',
              editable: true,
              effective_value: 4096,
              is_overridden: true,
              label: 'Context Length',
              override_value: 4096,
              pmid: 'runtime.ggml.backends.llama.context_length',
              schema: {
                default_value: null,
                minimum: 0,
                type: 'integer',
              },
              search_terms: [],
            },
          ],
          title: 'Llama',
        },
      ],
      title: 'Runtime',
    },
  ] as SettingsDocumentResponse['sections'];
}

function localizedSettingsSections(): SettingsDocumentResponse['sections'] {
  return [
    {
      description_md: 'Runtime preferences used by the desktop shell.',
      i18n: serverI18n({
        description_md: 'server.settings.sections.runtime.description',
        title: 'server.settings.sections.runtime.title',
      }),
      id: 'runtime',
      subsections: [
        {
          description_md: 'Overrides specific to the GGML llama worker.',
          i18n: serverI18n({
            description_md: 'server.settings.subsections.runtime.llama.description',
            title: 'server.settings.subsections.runtime.llama.title',
          }),
          id: 'llama',
          properties: [
            {
              description_md: 'Override the llama context window length in tokens.',
              editable: true,
              effective_value: 4096,
              i18n: serverI18n({
                description_md: 'server.settings.properties.description.genericContextLength',
                label: 'server.settings.properties.label.genericContextLength',
              }),
              is_overridden: true,
              label: 'Context Length',
              override_value: 4096,
              pmid: 'runtime.ggml.backends.llama.context_length',
              schema: {
                default_value: null,
                minimum: 0,
                type: 'integer',
              },
              search_terms: [],
            },
            {
              description_md: 'Structured list of remote providers, credentials, and request defaults.',
              editable: true,
              effective_value: [],
              i18n: serverI18n({
                description_md: 'server.settings.properties.description.providerRegistry',
                label: 'server.settings.properties.label.providerRegistry',
              }),
              is_overridden: false,
              label: 'Provider Registry',
              override_value: null,
              pmid: 'providers.registry',
              schema: {
                default_value: [],
                json_schema: providerRegistrySchema(),
                type: 'array',
              },
              search_terms: [],
            },
            {
              description_md: 'Persistent stdio MCP server launch configurations.',
              editable: true,
              effective_value: [
                {
                  args: [],
                  command: 'node',
                  cwd: null,
                  enabled: true,
                  env: {},
                  name: 'memory',
                },
              ],
              i18n: serverI18n({
                description_md: 'server.settings.properties.description.mcpServers',
                label: 'server.settings.properties.label.mcpServers',
              }),
              is_overridden: false,
              label: 'MCP Servers',
              override_value: null,
              pmid: 'agent.tools.mcp.servers',
              schema: {
                default_value: [],
                json_schema: mcpServersSchema(),
                type: 'array',
              },
              search_terms: [],
            },
          ],
          title: 'Llama',
        },
      ],
      title: 'Runtime',
    },
  ] as SettingsDocumentResponse['sections'];
}

function serverI18n(fields: Record<string, string>) {
  return Object.fromEntries(
    Object.entries(fields).map(([field, key]) => [field, { key }]),
  );
}

function schemaI18n(fields: Record<string, string>) {
  return serverI18n(fields);
}

function providerRegistrySchema() {
  return {
    type: 'array',
    title: 'Provider Registry',
    'x-i18n': schemaI18n({
      title: 'server.settings.properties.label.providerRegistry',
    }),
    items: {
      type: 'object',
      title: 'Provider Entry',
      'x-i18n': schemaI18n({
        title: 'server.settings.schemas.provider.entry.title',
      }),
      properties: {
        api_base: schemaField('string', 'API Base URL', 'server.settings.schemas.provider.apiBase.title'),
        auth: {
          type: 'object',
          title: 'Authentication',
          properties: {
            api_key: schemaField('string', 'API Key', 'server.settings.schemas.provider.apiKey.title'),
            api_key_env: schemaField(
              'string',
              'API Key Environment Variable',
              'server.settings.schemas.provider.apiKeyEnv.title',
            ),
          },
        },
        defaults: {
          type: 'object',
          title: 'Request Defaults',
          properties: {
            headers: schemaField('object', 'Headers', 'server.settings.schemas.provider.headers.title'),
            query: schemaField('object', 'Query Parameters', 'server.settings.schemas.provider.query.title'),
          },
        },
        display_name: schemaField('string', 'Display Name', 'server.settings.schemas.provider.displayName.title'),
        family: {
          ...schemaField('string', 'Provider Family', 'server.settings.schemas.provider.family.title'),
          enum: ['openai_compatible'],
        },
        id: {
          ...schemaField('string', 'Provider ID', 'server.settings.schemas.provider.id.title'),
          description: 'Stable provider identifier.',
          'x-i18n': schemaI18n({
            description: 'server.settings.schemas.provider.id.description',
            title: 'server.settings.schemas.provider.id.title',
          }),
        },
      },
    },
  };
}

function mcpServersSchema() {
  return {
    type: 'array',
    title: 'MCP Servers',
    'x-i18n': schemaI18n({
      title: 'server.settings.properties.label.mcpServers',
    }),
    items: {
      type: 'object',
      title: 'MCP Server',
      'x-i18n': schemaI18n({
        title: 'server.settings.schemas.mcp.server.title',
      }),
      properties: {
        args: {
          type: 'array',
          title: 'Arguments',
          'x-i18n': schemaI18n({
            title: 'server.settings.schemas.mcp.args.title',
          }),
          items: schemaField('string', 'Entry', 'server.settings.schemas.stringEntry.title'),
        },
        command: {
          ...schemaField('string', 'Command', 'server.settings.schemas.mcp.command.title'),
          description: 'Executable used to launch the stdio MCP server.',
          'x-i18n': schemaI18n({
            description: 'server.settings.schemas.mcp.command.description',
            title: 'server.settings.schemas.mcp.command.title',
          }),
        },
        cwd: schemaField('string', 'Working Directory', 'server.settings.schemas.mcp.cwd.title'),
        enabled: schemaField('boolean', 'Enabled', 'server.settings.schemas.mcp.enabled.title'),
        env: {
          type: 'object',
          title: 'Environment Variable References',
          'x-i18n': schemaI18n({
            title: 'server.settings.schemas.mcp.env.title',
          }),
          additionalProperties: {
            type: 'object',
            title: 'Environment Reference',
            'x-i18n': schemaI18n({
              title: 'server.settings.schemas.mcp.envReference.title',
            }),
            properties: {
              env_var: schemaField('string', 'Host Environment Variable', 'server.settings.schemas.mcp.envVar.title'),
            },
          },
        },
        name: {
          ...schemaField('string', 'Server Name', 'server.settings.schemas.mcp.name.title'),
          description: 'Stable local name used to route MCP tool calls.',
          'x-i18n': schemaI18n({
            description: 'server.settings.schemas.mcp.name.description',
            title: 'server.settings.schemas.mcp.name.title',
          }),
        },
      },
      required: ['name', 'command'],
    },
  };
}

function schemaField(type: string, title: string, titleKey: string) {
  return {
    type,
    title,
    'x-i18n': schemaI18n({
      title: titleKey,
    }),
  };
}
