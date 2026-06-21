import { describe, expect, it } from 'vitest';
import {
  ApiError,
  ErrorCodes,
  NetworkError,
  TimeoutError,
  errorMiddleware,
  getErrorData,
  getErrorCode,
  getErrorMessage,
  getLocalizedErrorMessage,
  isApiErrorResponse,
  isApiError,
  isRetryable,
} from '../errors';

const translate = (key: string, options?: Record<string, unknown>) => {
  if (key === 'server.errors.badRequest') {
    return `translated ${options?.detail}`;
  }

  return typeof options?.defaultValue === 'string' ? options.defaultValue : key;
};

describe('ApiError', () => {
  describe('constructor', () => {
    it('should create error with basic parameters', () => {
      const error = new ApiError(4000, 'Bad request');

      expect(error.name).toBe('ApiError');
      expect(error.code).toBe(4000);
      expect(error.message).toBe('Bad request');
      expect(error.status).toBeUndefined();
      expect(error.data).toBeUndefined();
    });

    it('should create error with all parameters', () => {
      const data = { field: 'value' };
      const error = new ApiError(4000, 'Bad request', data, 400);

      expect(error.code).toBe(4000);
      expect(error.message).toBe('Bad request');
      expect(error.data).toEqual(data);
      expect(error.status).toBe(400);
    });
  });

  describe('fromResponse', () => {
    it('should create error from standard error response', () => {
      const response = new Response(null, { status: 400 });
      const errorData = { code: 4000, message: 'Invalid input', data: null };

      const error = ApiError.fromResponse(response, errorData);

      expect(error.code).toBe(4000);
      expect(error.message).toBe('Invalid input');
      expect(error.status).toBe(400);
    });

    it('should create error from response with status', () => {
      const response = new Response(null, { status: 404 });
      const errorData = { code: 4004, message: 'Not found' };

      const error = ApiError.fromResponse(response, errorData);

      expect(error.code).toBe(4004);
      expect(error.status).toBe(404);
    });

    it('should create error from non-standard response', () => {
      const response = new Response(null, { status: 500, statusText: 'Internal Server Error' });
      const errorData = { random: 'data' };

      const error = ApiError.fromResponse(response, errorData);

      expect(error.code).toBe(5000);
      expect(error.message).toBe('500 Internal Server Error');
      expect(error.status).toBe(500);
    });

    it('should create actionable errors for unauthorized responses', () => {
      const response = new Response(null, { status: 401, statusText: 'Unauthorized' });

      const error = ApiError.fromResponse(response);

      expect(error.code).toBe(ErrorCodes.UNAUTHORIZED);
      expect(error.getUserMessage()).toContain('server.admin.token');
      expect(error.status).toBe(401);
    });
  });

  describe('isClientError', () => {
    it('should return true for 4xxx codes', () => {
      const error = new ApiError(4000, 'Bad request');
      expect(error.isClientError()).toBe(true);
    });

    it('should return false for 5xxx codes', () => {
      const error = new ApiError(5000, 'Server error');
      expect(error.isClientError()).toBe(false);
    });
  });

  describe('isServerError', () => {
    it('should return true for 5xxx codes', () => {
      const error = new ApiError(5000, 'Server error');
      expect(error.isServerError()).toBe(true);
    });

    it('should return false for 4xxx codes', () => {
      const error = new ApiError(4000, 'Bad request');
      expect(error.isServerError()).toBe(false);
    });
  });

  describe('getUserMessage', () => {
    it('should return custom message when present', () => {
      const error = new ApiError(4000, 'Custom error message');
      expect(error.getUserMessage()).toBe('Custom error message');
    });

    it('should return generic message for known error codes', () => {
      const error = new ApiError(4004, 'error: Not found');
      expect(error.getUserMessage()).toBe('The requested resource was not found.');
    });

    it('should return custom message for unknown error codes', () => {
      const error = new ApiError(9999, 'Unknown error');
      expect(error.getUserMessage()).toBe('Unknown error');
    });

    it.each([
      [4000, 'Invalid request. Please check your input and try again.'],
      [4010, 'Admin API authorization failed. Configure server.admin.token or provide the matching bearer token.'],
      [4009, 'The request conflicts with the current state. Refresh and try again.'],
      [4029, 'Too many requests. Wait a moment and try again.'],
      [5000, 'An error occurred while processing your request.'],
      [5001, 'A database error occurred. Please try again later.'],
      [5002, 'An internal server error occurred. Please try again later.'],
      [5003, 'Backend service is not ready. Please ensure all backends are properly configured.'],
      [5010, 'This operation is not implemented yet.'],
      [9999, 'An unexpected error occurred. Please try again.'],
    ])('should return fallback message for code %s', (code, message) => {
      expect(new ApiError(code, 'error: backend detail').getUserMessage()).toBe(message);
    });
  });
});

describe('specialized API errors', () => {
  it('creates network and timeout errors with API error semantics', () => {
    expect(new NetworkError()).toMatchObject({
      code: 5002,
      message: 'Network request failed',
      name: 'NetworkError',
    });
    expect(new TimeoutError('slow request')).toMatchObject({
      code: 5002,
      message: 'slow request',
      name: 'TimeoutError',
    });
  });
});

describe('errorMiddleware', () => {
  it('lets successful responses pass through', async () => {
    await expect(
      errorMiddleware.onResponse?.({ response: new Response('ok') } as never),
    ).resolves.toBeUndefined();
  });

  it('throws ApiError with backend code, data, and HTTP status', async () => {
    await expect(
      errorMiddleware.onResponse?.({
        response: new Response(
          JSON.stringify({ code: 4000, message: 'Invalid input', data: { path: 'name' } }),
          {
            status: 400,
            headers: { 'content-type': 'application/json' },
          },
        ),
      } as never),
    ).rejects.toMatchObject({
      code: 4000,
      data: { path: 'name' },
      message: 'Invalid input',
      status: 400,
    });
  });

  it('throws actionable ApiError for unauthorized responses', async () => {
    await expect(
      errorMiddleware.onResponse?.({
        response: new Response('', {
          status: 401,
          statusText: 'Unauthorized',
        }),
      } as never),
    ).rejects.toMatchObject({
      code: ErrorCodes.UNAUTHORIZED,
      message: expect.stringContaining('server.admin.token'),
      status: 401,
    });
  });

  it('throws a generic error for malformed JSON error bodies', async () => {
    await expect(
      errorMiddleware.onResponse?.({
        response: new Response('not json', {
          status: 502,
          statusText: 'Bad Gateway',
        }),
      } as never),
    ).rejects.toThrow('502 Bad Gateway');
  });

  it('wraps fetch errors and non-error throwables', async () => {
    await expect(errorMiddleware.onError?.({ error: new Error('offline') } as never)).resolves.toEqual(
      new ApiError(5002, 'offline'),
    );
    await expect(errorMiddleware.onError?.({ error: 'offline' } as never)).resolves.toEqual(
      new Error('offline'),
    );
  });
});

describe('ErrorCodes', () => {
  it('should have correct error code values', () => {
    expect(ErrorCodes.NOT_FOUND).toBe(4004);
    expect(ErrorCodes.UNAUTHORIZED).toBe(4010);
    expect(ErrorCodes.BAD_REQUEST).toBe(4000);
    expect(ErrorCodes.CONFLICT).toBe(4009);
    expect(ErrorCodes.TOO_MANY_REQUESTS).toBe(4029);
    expect(ErrorCodes.BACKEND_NOT_READY).toBe(5003);
    expect(ErrorCodes.RUNTIME_ERROR).toBe(5000);
    expect(ErrorCodes.DATABASE_ERROR).toBe(5001);
    expect(ErrorCodes.INTERNAL_ERROR).toBe(5002);
    expect(ErrorCodes.NOT_IMPLEMENTED).toBe(5010);
  });
});

describe('localized and structured error helpers', () => {
  it('recognizes backend error envelopes and translates message fields', () => {
    const payload = {
      code: ErrorCodes.BAD_REQUEST,
      data: { code: 'unsupported_chat_parameter', error_type: 'invalid_request_error', param: 'top_k' },
      i18n: {
        message: {
          key: 'server.errors.badRequest',
          params: { detail: 'top_k is unsupported' },
        },
      },
      message: 'top_k is unsupported',
    };

    expect(isApiErrorResponse(payload)).toBe(true);
    expect(getLocalizedErrorMessage(payload, translate)).toBe('translated top_k is unsupported');
    expect(getErrorData(payload)).toEqual(payload.data);
  });

  it('returns retryability for transient server-side codes only', () => {
    expect(isRetryable(new ApiError(ErrorCodes.TOO_MANY_REQUESTS, 'busy', null, 429))).toBe(true);
    expect(isRetryable(new ApiError(ErrorCodes.BACKEND_NOT_READY, 'warming', null, 503))).toBe(true);
    expect(isRetryable(new ApiError(ErrorCodes.CONFLICT, 'conflict', null, 409))).toBe(false);
    expect(isRetryable(new ApiError(ErrorCodes.NOT_IMPLEMENTED, 'missing', null, 501))).toBe(false);
    expect(isRetryable(new NetworkError())).toBe(true);
    expect(isRetryable(new TimeoutError())).toBe(true);
  });
});

describe('getErrorMessage', () => {
  it('should return message for ApiError', () => {
    const error = new ApiError(4004, 'Not found');
    expect(getErrorMessage(error)).toBe('Not found');
  });

  it('should return message for regular Error', () => {
    const error = new Error('Something went wrong');
    expect(getErrorMessage(error)).toBe('Something went wrong');
  });

  it('should return string representation for non-Error objects', () => {
    expect(getErrorMessage('string error')).toBe('string error');
    expect(getErrorMessage(123)).toBe('123');
  });
});

describe('getErrorCode', () => {
  it('should return code for ApiError', () => {
    const error = new ApiError(4000, 'Bad request');
    expect(getErrorCode(error)).toBe(4000);
  });

  it('should return undefined for regular Error', () => {
    const error = new Error('Something went wrong');
    expect(getErrorCode(error)).toBeUndefined();
  });

  it('should return undefined for non-Error objects', () => {
    expect(getErrorCode('string error')).toBeUndefined();
  });
});

describe('isApiError', () => {
  it('should return true for ApiError instances', () => {
    const error = new ApiError(4000, 'Bad request');
    expect(isApiError(error)).toBe(true);
  });

  it('should return false for regular Error instances', () => {
    const error = new Error('Something went wrong');
    expect(isApiError(error)).toBe(false);
  });

  it('should return false for non-Error objects', () => {
    expect(isApiError('string error')).toBe(false);
    expect(isApiError(null)).toBe(false);
    expect(isApiError(undefined)).toBe(false);
  });
});
