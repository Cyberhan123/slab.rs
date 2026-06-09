import { useCallback, useEffect, useState } from 'react';
import { orderBy } from 'lodash-es';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { getErrorMessage } from '@slab/api';
import {
  getAudioTranscription,
  listAudioTranscriptions,
  type AudioTranscriptionTask,
} from '@/lib/media-task-api';

export function useAudioHistory() {
  const { t } = useTranslation();
  const [history, setHistory] = useState<AudioTranscriptionTask[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [selectedHistoryTask, setSelectedHistoryTask] = useState<AudioTranscriptionTask | null>(null);
  const [historyDialogOpen, setHistoryDialogOpen] = useState(false);

  const showHistoryTask = useCallback((task: AudioTranscriptionTask) => {
    setHistory((previous) => {
      const next = [task, ...previous.filter((entry) => entry.task_id !== task.task_id)];
      return orderBy(next, (entry) => Date.parse(entry.created_at), 'desc');
    });
    setSelectedHistoryTask(task);
    setHistoryDialogOpen(true);
  }, []);

  const refreshHistory = useCallback(async () => {
    try {
      setHistoryLoading(true);
      setHistoryError(null);
      const items = await listAudioTranscriptions();
      setHistory(items);
    } catch (error) {
      setHistoryError(getErrorMessage(error));
    } finally {
      setHistoryLoading(false);
    }
  }, []);

  const openHistoryDetail = useCallback(async (taskIdToOpen: string) => {
    try {
      const detail = await getAudioTranscription(taskIdToOpen);
      showHistoryTask(detail);
    } catch (error) {
      const message = getErrorMessage(error);
      toast.error(t('pages.audio.toast.historyDetailFailed', { message }));
    }
  }, [showHistoryTask, t]);

  useEffect(() => {
    void refreshHistory();
  }, [refreshHistory]);

  return {
    history,
    historyDialogOpen,
    historyError,
    historyLoading,
    openHistoryDetail,
    selectedHistoryTask,
    setHistoryDialogOpen,
    setSelectedHistoryTask,
    showHistoryTask,
  };
}
