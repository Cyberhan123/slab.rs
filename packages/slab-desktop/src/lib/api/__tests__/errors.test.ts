import { describe, it, expect } from 'vitest';
import { ApiError, ErrorCodes, getErrorMessage, getErrorCode, isApiError } from '../errors';

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
  });
});

describe('ErrorCodes', () => {
  it('should have correct error code values', () => {
    expect(ErrorCodes.NOT_FOUND).toBe(4004);
    expect(ErrorCodes.BAD_REQUEST).toBe(4000);
    expect(ErrorCodes.BACKEND_NOT_READY).toBe(5003);
    expect(ErrorCodes.RUNTIME_ERROR).toBe(5000);
    expect(ErrorCodes.DATABASE_ERROR).toBe(5001);
    expect(ErrorCodes.INTERNAL_ERROR).toBe(5002);
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
