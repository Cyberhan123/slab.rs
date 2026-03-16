import { Plus, Sparkles, Trash2 } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

import { isJsonObject, pathContainsError } from '../schema';
import type { FieldErrorState, JsonValue, SettingResponse } from '../types';

const CHAT_PROVIDERS_PMID = 'chat.providers';

type EditableProviderModel = {
  id: string;
  display_name: string;
  remote_model: string;
};

type EditableProvider = {
  id: string;
  name: string;
  api_base: string;
  api_key: string;
  api_key_env: string;
  models: EditableProviderModel[];
};

type ChatProvidersFieldProps = {
  value: JsonValue;
  errorState?: FieldErrorState;
  onChange: (value: JsonValue) => void;
};

type FieldInputProps = {
  id: string;
  label: string;
  value: string;
  placeholder: string;
  description?: string;
  type?: 'password' | 'text';
  error?: string;
  onChange: (value: string) => void;
};

export function supportsChatProvidersField(
  property: Pick<SettingResponse, 'pmid' | 'schema'>,
): boolean {
  return property.pmid === CHAT_PROVIDERS_PMID && property.schema.type === 'array';
}

export function ChatProvidersField({
  value,
  errorState,
  onChange,
}: ChatProvidersFieldProps) {
  const providers = toEditableProviders(value);
  const rootError = errorState?.path === '/' ? errorState.message : null;

  function commit(nextProviders: EditableProvider[]) {
    onChange(nextProviders as JsonValue);
  }

  function addProvider() {
    commit([...providers, createProviderDraft(providers)]);
  }

  function updateProvider(index: number, nextProvider: EditableProvider) {
    commit(
      providers.map((provider, providerIndex) =>
        providerIndex === index ? nextProvider : provider,
      ),
    );
  }

  function removeProvider(index: number) {
    commit(providers.filter((_, providerIndex) => providerIndex !== index));
  }

  return (
    <div className="space-y-4 rounded-3xl border border-border/70 bg-muted/10 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3 rounded-2xl border border-border/60 bg-background/70 px-4 py-3">
        <div className="space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-sm font-semibold">OpenAI-compatible providers</p>
            <Badge variant="secondary">{providers.length} configured</Badge>
          </div>
          <p className="text-xs leading-5 text-muted-foreground">
            Configure the endpoint, credentials, and model ids exposed in the chat
            model picker. New entries start with safe starter values so auto-save can
            keep working while you edit.
          </p>
        </div>

        <Button variant="outline" size="sm" onClick={addProvider}>
          <Plus className="mr-2 h-4 w-4" />
          Add provider
        </Button>
      </div>

      {providers.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-border/70 bg-background/70 px-5 py-6 text-sm">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="space-y-1">
              <p className="font-medium">No cloud providers configured yet.</p>
              <p className="text-muted-foreground">
                Add a provider to expose OpenAI-compatible cloud models in chat.
              </p>
            </div>
            <Button onClick={addProvider}>
              <Plus className="mr-2 h-4 w-4" />
              Add first provider
            </Button>
          </div>
        </div>
      ) : null}

      <div className="space-y-4">
        {providers.map((provider, providerIndex) => {
          const providerPath = `/${providerIndex}`;
          const providerLabel =
            provider.name.trim() || provider.id.trim() || `Provider ${providerIndex + 1}`;
          const providerHasError = pathContainsError(providerPath, errorState?.path);

          return (
            <div
              key={`provider-${providerIndex}`}
              className={cn(
                'space-y-4 rounded-2xl border border-border/70 bg-background/85 p-4 shadow-[0_16px_40px_-34px_color-mix(in_oklab,var(--foreground)_32%,transparent)]',
                providerHasError && 'border-destructive/60 bg-destructive/5',
              )}
            >
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="space-y-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <h4 className="text-sm font-semibold">{providerLabel}</h4>
                    <Badge variant="outline">
                      {provider.models.length} model
                      {provider.models.length === 1 ? '' : 's'}
                    </Badge>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    Generated ids are just starters. Replace them with the exact provider
                    and model identifiers you want users to select.
                  </p>
                </div>

                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => removeProvider(providerIndex)}
                >
                  <Trash2 className="mr-2 h-4 w-4" />
                  Remove provider
                </Button>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <FieldInput
                  id={`chat-provider-${providerIndex}-id`}
                  label="Provider ID"
                  value={provider.id}
                  placeholder="openai-main"
                  description="Stable internal id used for the cloud provider."
                  error={errorMessageAtPath(errorState, `${providerPath}/id`)}
                  onChange={(nextValue) =>
                    updateProvider(providerIndex, {
                      ...provider,
                      id: nextValue,
                    })
                  }
                />
                <FieldInput
                  id={`chat-provider-${providerIndex}-name`}
                  label="Display name"
                  value={provider.name}
                  placeholder="OpenAI"
                  description="Optional. Leave empty to reuse the provider id."
                  error={errorMessageAtPath(errorState, `${providerPath}/name`)}
                  onChange={(nextValue) =>
                    updateProvider(providerIndex, {
                      ...provider,
                      name: nextValue,
                    })
                  }
                />
                <FieldInput
                  id={`chat-provider-${providerIndex}-api-base`}
                  label="API base URL"
                  value={provider.api_base}
                  placeholder="https://api.openai.com/v1"
                  description="The OpenAI-compatible base endpoint for this provider."
                  error={errorMessageAtPath(errorState, `${providerPath}/api_base`)}
                  onChange={(nextValue) =>
                    updateProvider(providerIndex, {
                      ...provider,
                      api_base: nextValue,
                    })
                  }
                />
                <FieldInput
                  id={`chat-provider-${providerIndex}-api-key-env`}
                  label="API key env var"
                  value={provider.api_key_env}
                  placeholder="OPENAI_API_KEY"
                  description="Optional. Read the secret from an environment variable."
                  error={errorMessageAtPath(errorState, `${providerPath}/api_key_env`)}
                  onChange={(nextValue) =>
                    updateProvider(providerIndex, {
                      ...provider,
                      api_key_env: nextValue,
                    })
                  }
                />
                <div className="md:col-span-2">
                  <FieldInput
                    id={`chat-provider-${providerIndex}-api-key`}
                    label="API key"
                    value={provider.api_key}
                    type="password"
                    placeholder="sk-..."
                    description="Optional. Store the secret directly when an env var is not available."
                    error={errorMessageAtPath(errorState, `${providerPath}/api_key`)}
                    onChange={(nextValue) =>
                      updateProvider(providerIndex, {
                        ...provider,
                        api_key: nextValue,
                      })
                    }
                  />
                </div>
              </div>

              <div className="space-y-3 rounded-2xl border border-border/60 bg-muted/10 p-4">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="space-y-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-sm font-semibold">Models</p>
                      <Badge variant="secondary">
                        {provider.models.length} configured
                      </Badge>
                    </div>
                    <p className="text-xs leading-5 text-muted-foreground">
                      Each model appears in the chat selector. Leave remote model empty
                      to reuse the local model id.
                    </p>
                  </div>

                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      updateProvider(providerIndex, {
                        ...provider,
                        models: [...provider.models, createModelDraft(provider.models)],
                      })
                    }
                  >
                    <Plus className="mr-2 h-4 w-4" />
                    Add model
                  </Button>
                </div>

                {provider.models.length === 0 ? (
                  <div className="rounded-2xl border border-dashed border-border/70 bg-background/70 px-4 py-4 text-sm text-muted-foreground">
                    This provider needs at least one model.
                  </div>
                ) : null}

                <div className="space-y-3">
                  {provider.models.map((model, modelIndex) => {
                    const modelPath = `${providerPath}/models/${modelIndex}`;
                    const modelLabel =
                      model.display_name.trim() ||
                      model.id.trim() ||
                      `Model ${modelIndex + 1}`;
                    const modelHasError = pathContainsError(modelPath, errorState?.path);

                    return (
                      <div
                        key={`provider-${providerIndex}-model-${modelIndex}`}
                        className={cn(
                          'space-y-4 rounded-2xl border border-border/70 bg-background/85 p-4',
                          modelHasError && 'border-destructive/60 bg-destructive/5',
                        )}
                      >
                        <div className="flex flex-wrap items-start justify-between gap-3">
                          <div className="space-y-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <p className="text-sm font-medium">{modelLabel}</p>
                              <Badge variant="outline">
                                Slot {modelIndex + 1}
                              </Badge>
                            </div>
                            <p className="text-xs text-muted-foreground">
                              Keep the model id stable. Chat sessions reference this id.
                            </p>
                          </div>

                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() =>
                              updateProvider(providerIndex, {
                                ...provider,
                                models: provider.models.filter(
                                  (_, currentIndex) => currentIndex !== modelIndex,
                                ),
                              })
                            }
                            disabled={provider.models.length <= 1}
                          >
                            <Trash2 className="mr-2 h-4 w-4" />
                            Remove model
                          </Button>
                        </div>

                        <div className="grid gap-4 md:grid-cols-2">
                          <FieldInput
                            id={`chat-provider-${providerIndex}-model-${modelIndex}-id`}
                            label="Model ID"
                            value={model.id}
                            placeholder="gpt-4.1-mini"
                            description="Stable model id shown inside Slab."
                            error={errorMessageAtPath(errorState, `${modelPath}/id`)}
                            onChange={(nextValue) =>
                              updateProvider(providerIndex, {
                                ...provider,
                                models: provider.models.map((item, currentIndex) =>
                                  currentIndex === modelIndex
                                    ? {
                                        ...item,
                                        id: nextValue,
                                      }
                                    : item,
                                ),
                              })
                            }
                          />
                          <FieldInput
                            id={`chat-provider-${providerIndex}-model-${modelIndex}-display-name`}
                            label="Display name"
                            value={model.display_name}
                            placeholder="GPT-4.1 mini"
                            description="Optional. Leave empty to reuse the model id."
                            error={errorMessageAtPath(
                              errorState,
                              `${modelPath}/display_name`,
                            )}
                            onChange={(nextValue) =>
                              updateProvider(providerIndex, {
                                ...provider,
                                models: provider.models.map((item, currentIndex) =>
                                  currentIndex === modelIndex
                                    ? {
                                        ...item,
                                        display_name: nextValue,
                                      }
                                    : item,
                                ),
                              })
                            }
                          />
                          <div className="md:col-span-2">
                            <FieldInput
                              id={`chat-provider-${providerIndex}-model-${modelIndex}-remote-model`}
                              label="Remote model ID"
                              value={model.remote_model}
                              placeholder="gpt-4.1-mini"
                              description="Optional. Override the upstream model id if it differs from the Slab model id."
                              error={errorMessageAtPath(
                                errorState,
                                `${modelPath}/remote_model`,
                              )}
                              onChange={(nextValue) =>
                                updateProvider(providerIndex, {
                                  ...provider,
                                  models: provider.models.map((item, currentIndex) =>
                                    currentIndex === modelIndex
                                      ? {
                                          ...item,
                                          remote_model: nextValue,
                                        }
                                      : item,
                                  ),
                                })
                              }
                            />
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {rootError ? (
        <p className="flex items-center gap-2 text-sm text-destructive">
          <Sparkles className="h-4 w-4" />
          {rootError}
        </p>
      ) : null}
    </div>
  );
}

function FieldInput({
  id,
  label,
  value,
  placeholder,
  description,
  type = 'text',
  error,
  onChange,
}: FieldInputProps) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        type={type}
        value={value}
        placeholder={placeholder}
        aria-invalid={Boolean(error)}
        className={cn(Boolean(error) && 'border-destructive/70')}
        onChange={(event) => onChange(event.target.value)}
      />
      {description ? (
        <p className="text-xs leading-5 text-muted-foreground">{description}</p>
      ) : null}
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
    </div>
  );
}

function toEditableProviders(value: JsonValue): EditableProvider[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter(isJsonObject).map((provider) => ({
    id: readString(provider.id),
    name: readString(provider.name),
    api_base: readString(provider.api_base),
    api_key: readString(provider.api_key),
    api_key_env: readString(provider.api_key_env),
    models: Array.isArray(provider.models)
      ? provider.models.filter(isJsonObject).map((model) => ({
          id: readString(model.id),
          display_name: readString(model.display_name),
          remote_model: readString(model.remote_model),
        }))
      : [],
  }));
}

function readString(value: JsonValue | undefined): string {
  return typeof value === 'string' ? value : '';
}

function createProviderDraft(providers: EditableProvider[]): EditableProvider {
  return {
    id: nextGeneratedId(
      providers.map((provider) => provider.id),
      'provider',
    ),
    name: '',
    api_base: 'https://api.openai.com/v1',
    api_key: '',
    api_key_env: '',
    models: [createModelDraft([])],
  };
}

function createModelDraft(models: EditableProviderModel[]): EditableProviderModel {
  return {
    id: nextGeneratedId(
      models.map((model) => model.id),
      'model',
    ),
    display_name: '',
    remote_model: '',
  };
}

function nextGeneratedId(existingIds: string[], prefix: string): string {
  const usedIds = new Set(existingIds.map((value) => value.trim()).filter(Boolean));
  let counter = usedIds.size + 1;

  while (usedIds.has(`${prefix}-${counter}`)) {
    counter += 1;
  }

  return `${prefix}-${counter}`;
}

function errorMessageAtPath(
  errorState: FieldErrorState | undefined,
  path: string,
): string | undefined {
  return errorState?.path === path ? errorState.message : undefined;
}
