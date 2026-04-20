import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import TaskPage from '@/pages/task';
import type { Task } from '@/pages/task/const';
import { renderDesktopScene } from '../test-utils';

const { mockUseTaskList } = vi.hoisted(() => ({
  mockUseTaskList: vi.fn<() => unknown>(),
}));

vi.mock('@/pages/task/hooks/use-task-list', () => ({
  useTaskList: mockUseTaskList,
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

vi.mock('@slab/i18n', () => ({
  useTranslation: vi.fn(() => ({
    t: vi.fn((key: string) => key),
    i18n: {
      resolvedLanguage: 'en',
      language: 'en',
    },
  })),
}));

function createMockTask(overrides: Partial<Task> = {}): Task {
  return {
    id: 'task-abc-123-def-456',
    task_type: 'image_generation',
    status: 'succeeded',
    created_at: '2024-01-15T10:30:00Z',
    updated_at: '2024-01-15T10:31:00Z',
    ...overrides,
  };
}

function createTaskViewModel(overrides = {}) {
  return {
    allTasks: [] as Task[],
    metrics: {
      total: 0,
      running: 0,
      queued: 0,
      failed: 0,
      succeeded: 0,
    },
    settledTasks: [] as Task[],
    successRate: 0,
    activeTaskCount: 0,
    activeShare: 0,
    averageTurnaroundMs: 0,
    successSparkline: [0.32, 0.48, 0.44, 0.66, 0.82, 0.72, 0.77],
    durationSparkline: [0.18, 0.28, 0.24, 0.6, 0.44],
    totalPages: 1,
    currentPage: 1,
    paginatedTasks: [] as Task[],
    paginationLabel: 'pages.task.table.pagination.empty',
    selectedTask: null,
    setSelectedTask: vi.fn(),
    taskResult: null,
    tasksError: null,
    tasksLoading: false,
    cancelTaskMutation: {
      isPending: false,
      mutateAsync: vi.fn().mockResolvedValue(undefined),
    },
    restartTaskMutation: {
      isPending: false,
      mutateAsync: vi.fn().mockResolvedValue(undefined),
    },
    fetchTaskDetail: vi.fn().mockResolvedValue(undefined),
    cancelTask: vi.fn().mockResolvedValue(undefined),
    restartTask: vi.fn().mockResolvedValue(undefined),
    setPage: vi.fn(),
    ...overrides,
  };
}

describe('TaskPage browser visual regression', () => {
  beforeEach(() => {
    mockUseTaskList.mockReset();
  });

  it('captures the task page empty state', async () => {
    mockUseTaskList.mockReturnValue(
      createTaskViewModel({
        allTasks: [],
        paginatedTasks: [],
        tasksLoading: false,
      }),
    );

    await renderDesktopScene(<TaskPage />, { route: '/task' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('task-page-empty.png');
  });

  it('captures the task page loading state', async () => {
    mockUseTaskList.mockReturnValue(
      createTaskViewModel({
        tasksLoading: true,
      }),
    );

    await renderDesktopScene(<TaskPage />, { route: '/task' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('task-page-loading.png');
  });

  it('captures the task page with tasks', async () => {
    const mockTasks: Task[] = [
      createMockTask({
        id: 'task-1',
        task_type: 'image_generation',
        status: 'succeeded',
      }),
      createMockTask({
        id: 'task-2',
        task_type: 'audio_transcription',
        status: 'running',
      }),
      createMockTask({
        id: 'task-3',
        task_type: 'video_generation',
        status: 'pending',
      }),
      createMockTask({
        id: 'task-4',
        task_type: 'image_generation',
        status: 'failed',
      }),
    ];

    mockUseTaskList.mockReturnValue(
      createTaskViewModel({
        allTasks: mockTasks,
        paginatedTasks: mockTasks,
        metrics: {
          total: 4,
          running: 1,
          queued: 1,
          failed: 1,
          succeeded: 1,
        },
        settledTasks: [mockTasks[0], mockTasks[3]],
        successRate: 50,
        activeTaskCount: 2,
        activeShare: 50,
        averageTurnaroundMs: 45000,
        paginationLabel: 'Showing 1-4 of 4 tasks',
        totalPages: 1,
        currentPage: 1,
      }),
    );

    await renderDesktopScene(<TaskPage />, { route: '/task' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('task-page-with-tasks.png');
  });

  it('captures the task page error state', async () => {
    mockUseTaskList.mockReturnValue(
      createTaskViewModel({
        tasksError: new Error('Failed to load tasks'),
      }),
    );

    await renderDesktopScene(<TaskPage />, { route: '/task' });

    await expect.element(page.getByTestId('desktop-browser-scene')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('task-page-error.png');
  });
});
