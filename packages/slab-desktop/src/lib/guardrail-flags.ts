import api from '@slab/api';

export const GUARDRAIL_PMIDS = {
  assistantErrorEnvelopeRendering: 'guardrails.assistant_error_envelope_rendering',
  assistantSseResume: 'guardrails.assistant_sse_resume',
  workspaceMonacoLazy: 'guardrails.workspace_monaco_lazy',
} as const;

export type GuardrailPmid = (typeof GUARDRAIL_PMIDS)[keyof typeof GUARDRAIL_PMIDS];

export function settingValueToEnabledFlag(value: unknown, defaultValue = true): boolean {
  return typeof value === 'boolean' ? value : defaultValue;
}

export function useGuardrailFlag(pmid: GuardrailPmid): boolean {
  const { data } = api.useQuery(
    'get',
    '/v1/settings/{pmid}',
    {
      params: {
        path: {
          pmid,
        },
      },
    },
    {
      refetchOnMount: false,
      refetchOnReconnect: true,
      refetchOnWindowFocus: true,
      retry: false,
      staleTime: 30_000,
    },
  );

  return settingValueToEnabledFlag(data?.effective_value);
}
