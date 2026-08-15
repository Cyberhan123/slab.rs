import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@slab/api', () => ({
  default: {
    useQuery: vi.fn<() => unknown>(),
  },
}));

import api from '@slab/api';
import { settingValueToEnabledFlag, useGuardrailFlag } from '../guardrail-flags';

const mockedApi = vi.mocked(api);

describe('guardrail flags', () => {
  it('defaults guardrail settings to enabled unless a boolean override is present', () => {
    expect(settingValueToEnabledFlag(true)).toBe(true);
    expect(settingValueToEnabledFlag(false)).toBe(false);
    expect(settingValueToEnabledFlag(null)).toBe(true);
    expect(settingValueToEnabledFlag(undefined)).toBe(true);
    expect(settingValueToEnabledFlag('false')).toBe(true);
  });
});

describe('useGuardrailFlag', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('queries the setting by pmid with retry disabled and a short stale time', () => {
    mockedApi.useQuery.mockReturnValue({ data: undefined });

    useGuardrailFlag('guardrails.assistant_sse_resume');

    expect(mockedApi.useQuery).toHaveBeenCalledWith(
      'get',
      '/v1/settings/{pmid}',
      { params: { path: { pmid: 'guardrails.assistant_sse_resume' } } },
      {
        refetchOnMount: false,
        refetchOnReconnect: true,
        refetchOnWindowFocus: true,
        retry: false,
        staleTime: 30_000,
      },
    );
  });

  it('returns the effective boolean when the setting provides one', () => {
    mockedApi.useQuery.mockReturnValue({ data: { effective_value: false } });
    expect(useGuardrailFlag('guardrails.assistant_sse_resume')).toBe(false);

    mockedApi.useQuery.mockReturnValue({ data: { effective_value: true } });
    expect(useGuardrailFlag('guardrails.assistant_sse_resume')).toBe(true);
  });

  it('defaults to enabled when the effective value is missing or non-boolean', () => {
    mockedApi.useQuery.mockReturnValue({ data: undefined });
    expect(useGuardrailFlag('guardrails.assistant_sse_resume')).toBe(true);

    mockedApi.useQuery.mockReturnValue({ data: { effective_value: 'false' } });
    expect(useGuardrailFlag('guardrails.assistant_sse_resume')).toBe(true);
  });
});
