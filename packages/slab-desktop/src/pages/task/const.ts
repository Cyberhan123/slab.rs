import type { components } from '@slab/api/v1';

export const PAGE_SIZE = 4;
export const TASK_LIST_POLL_INTERVAL_MS = 3_000;

export type Task = components['schemas']['TaskResponse'];
export type TaskResult = components['schemas']['TaskResultPayload'];
