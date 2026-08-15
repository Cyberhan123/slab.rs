import { describe, expect, it } from 'vitest';

import { ApiError, NetworkError } from '@slab/api';
import {
  isAbortLikeError,
  shouldRetryQuery,
  shouldShowGlobalMutationErrorToast,
  toErrorToastId,
} from '../query-client';

describe('query-client defaults', () => {
  it('retries retryable query errors only within the configured attempt budget', () => {
    expect(shouldRetryQuery(0, new NetworkError('offline'))).toBe(true);
    expect(shouldRetryQuery(1, new ApiError(5003, 'Backend not ready', null, 503))).toBe(true);
    expect(shouldRetryQuery(2, new ApiError(5003, 'Backend not ready', null, 503))).toBe(false);
    expect(shouldRetryQuery(0, new ApiError(4000, 'Bad request', null, 400))).toBe(false);
  });

  it('skips global mutation toasts for aborts and local-handled mutations', () => {
    expect(
      shouldShowGlobalMutationErrorToast(new Error('failed'), {
        skipGlobalErrorToast: true,
      }),
    ).toBe(false);
    expect(
      shouldShowGlobalMutationErrorToast(new DOMException('aborted', 'AbortError'), undefined),
    ).toBe(false);
    expect(shouldShowGlobalMutationErrorToast(new Error('failed'), undefined)).toBe(true);
  });

  it('builds stable mutation error toast ids', () => {
    expect(toErrorToastId(new Error('failed'))).toBe('mutation-error:Error:failed');
  });

  it('recognizes Error-shaped AbortError values', () => {
    const error = new Error('aborted');
    error.name = 'AbortError';
    expect(isAbortLikeError(error)).toBe(true);
  });
});
