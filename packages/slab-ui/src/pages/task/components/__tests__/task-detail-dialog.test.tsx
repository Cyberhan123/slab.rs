import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import { MemoryRouter } from 'react-router-dom';

import type { Task, TaskResult } from '../../const';
import { TaskDetailDialog } from '../task-detail-dialog';

vi.mock('@slab/i18n', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@slab/i18n')>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) => key,
      i18n: { resolvedLanguage: 'en', language: 'en' },
    }),
  };
});

vi.mock('@mantine/hooks', () => ({
  useClipboard: () => ({ copy: vi.fn<() => void>(), copied: false }),
}));

function buildTask(taskType: string, status: Task['status']): Task {
  return {
    id: 'task-1',
    task_type: taskType,
    status,
    error_msg: status === 'failed' ? 'boom' : null,
    i18n: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  } as unknown as Task;
}

async function renderDialog(task: Task, taskResult: TaskResult | null = null) {
  return render(
    <MemoryRouter>
      <TaskDetailDialog
        task={task}
        selectedTask={task}
        taskResult={taskResult}
        cancelTaskMutation={{ isPending: false }}
        restartTaskMutation={{ isPending: false }}
        onOpen={() => {}}
        onCancel={() => {}}
        onRestart={() => {}}
      />
    </MemoryRouter>,
  );
}

describe('TaskDetailDialog restart gating', () => {
  it('shows the Restart button for a failed model_download task', async () => {
    const screen = await renderDialog(buildTask('model_download', 'failed'));
    await screen.getByTestId('task-details-open-task-1').click();

    await expect.element(screen.getByTestId('task-restart-task-1')).toBeInTheDocument();
  });

  it('hides the Restart button for a failed image_generation task', async () => {
    const screen = await renderDialog(buildTask('image_generation', 'failed'));
    await screen.getByTestId('task-details-open-task-1').click();

    expect(screen.getByTestId('task-restart-task-1').query()).toBeNull();
  });

  it('hides the Restart button for a failed audio_transcription task', async () => {
    const screen = await renderDialog(buildTask('audio_transcription', 'failed'));
    await screen.getByTestId('task-details-open-task-1').click();

    expect(screen.getByTestId('task-restart-task-1').query()).toBeNull();
  });
});
