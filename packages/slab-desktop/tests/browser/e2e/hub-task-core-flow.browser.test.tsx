import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import HubPage from '@/pages/hub';
import TaskPage from '@/pages/task';
import { renderDesktopScene } from '../test-utils';

const {
  mockCancelTask,
  mockDeleteModel,
  mockDownloadModel,
  mockFetchTaskDetail,
} = vi.hoisted(() => ({
  mockCancelTask: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockDeleteModel: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockDownloadModel: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  mockFetchTaskDetail: vi.fn<(id: string) => Promise<void>>().mockResolvedValue(undefined),
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
}));

vi.mock('@/pages/hub/hooks/use-hub-model-catalog', async () => {
  const React = await import('react');

  const readyModel = {
    backend_ids: ['ggml.llama'],
    capabilities: ['chat_generation'],
    display_name: 'Llama 3.2 3B',
    download_progress: null,
    download_task_id: null,
    filename: 'llama-3.2-3b.gguf',
    id: 'llama-3b',
    is_vad_model: false,
    kind: 'local',
    local_path: 'C:/models/llama-3.2-3b.gguf',
    pending: false,
    repo_id: 'meta-llama/Llama-3.2-3B',
    status: 'ready',
    updated_at: '2026-06-01T10:00:00Z',
  };
  const downloadableModel = {
    ...readyModel,
    capabilities: ['image_generation'],
    display_name: 'Stable Diffusion XL',
    filename: 'sdxl.gguf',
    id: 'sdxl',
    local_path: null,
    repo_id: 'stabilityai/sdxl',
    status: 'not_downloaded',
  };

  return {
    CATEGORY_OPTIONS: ['all', 'language', 'vision', 'audio', 'coding', 'embedding'],
    STATUS_OPTIONS: ['all', 'ready', 'downloading', 'not_downloaded', 'error'],
    canDownloadModel: vi.fn<(model: { id?: string }) => boolean>((model) => model.id === 'sdxl'),
    useHubModelCatalog: vi.fn<() => unknown>(() => {
      const [modelToDelete, setModelToDelete] = React.useState<unknown>(null);
      const models = [readyModel, downloadableModel];

      return {
        canCreate: false,
        category: 'all',
        createFileName: null,
        createModel: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
        createModelPending: false,
        deleteModel: mockDeleteModel,
        deleteModelPending: false,
        downloadModel: mockDownloadModel,
        downloadedCount: 1,
        error: null,
        filteredModels: models,
        hasMore: false,
        isCreateOpen: false,
        isLoading: false,
        isRefetching: false,
        loadMore: vi.fn<() => void>(),
        modelToDelete,
        modelToEnhance: null,
        models,
        pendingCount: 0,
        refetch: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
        setCategory: vi.fn<() => void>(),
        setCreateFile: vi.fn<() => void>(),
        setCreateOpen: vi.fn<() => void>(),
        setModelToDelete,
        setModelToEnhance: vi.fn<() => void>(),
        setStatus: vi.fn<() => void>(),
        status: 'all',
        visibleModels: models,
      };
    }),
  };
});

vi.mock('@/pages/task/hooks/use-task-list', async () => {
  const React = await import('react');
  const runningTask = {
    created_at: '2026-06-01T10:00:00Z',
    error_msg: null,
    id: 'task-running-1',
    progress: null,
    status: 'running',
    task_type: 'agent_run',
    updated_at: '2026-06-01T10:01:00Z',
  };

  return {
    useTaskList: vi.fn<() => unknown>(() => {
      const [selectedTask, setSelectedTask] = React.useState<unknown>(null);

      return {
        activeShare: 100,
        activeTaskCount: 1,
        allTasks: [runningTask],
        averageTurnaroundMs: 0,
        cancelTask: mockCancelTask,
        cancelTaskMutation: { isPending: false },
        currentPage: 1,
        durationSparkline: [0.18, 0.28, 0.24, 0.6, 0.44],
        fetchTaskDetail: async (id: string) => {
          await mockFetchTaskDetail(id);
          setSelectedTask(runningTask);
        },
        metrics: {
          failed: 0,
          queued: 0,
          running: 1,
          succeeded: 0,
          total: 1,
        },
        paginatedTasks: [runningTask],
        paginationLabel: 'Showing 1 to 1 of 1 entries',
        restartTask: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
        restartTaskMutation: { isPending: false },
        selectedTask,
        setPage: vi.fn<() => void>(),
        setSelectedTask,
        settledTasks: [],
        successRate: 0,
        successSparkline: [0.66],
        taskResult: null,
        tasksError: null,
        tasksLoading: false,
        totalPages: 1,
      };
    }),
  };
});

describe('hub and task core flows e2e', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('starts a model download and confirms catalog deletion', async () => {
    await renderDesktopScene(<HubPage />, { route: '/hub' });

    await page.getByRole('button', { name: 'Download' }).click();
    expect(mockDownloadModel).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'sdxl',
      }),
    );

    await page.getByRole('button', { name: 'Delete Stable Diffusion XL' }).click();
    await expect.element(page.getByRole('alertdialog')).toBeVisible();
    await page.getByRole('button', { name: 'Delete entry' }).click();

    expect(mockDeleteModel).toHaveBeenCalled();
  });

  it('opens task details and cancels a running task', async () => {
    await renderDesktopScene(<TaskPage />, { route: '/task' });

    await page.getByRole('button', { name: 'Details' }).click();
    await expect.element(page.getByRole('dialog')).toBeVisible();
    await expect.element(page.getByText('Task ID: task-running-1')).toBeVisible();
    expect(mockFetchTaskDetail).toHaveBeenCalledWith('task-running-1');

    await page.getByRole('button', { name: 'Cancel task' }).click();
    expect(mockCancelTask).toHaveBeenCalledWith('task-running-1');
  });
});
