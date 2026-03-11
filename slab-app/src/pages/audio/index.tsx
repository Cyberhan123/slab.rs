import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Spinner } from '@/components/ui/spinner';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { toast } from 'sonner';
import useFile, { SelectedFile } from '@/hooks/use-file';
import useTranscribe from './hooks/use-transcribe';
import useIsTauri from '@/hooks/use-tauri';
import api from '@/lib/api';

const WHISPER_BACKEND_ID = 'ggml.whisper';
const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

type PreparingStage = 'prepare' | 'transcribe' | null;

export default function Audio() {
  const navigate = useNavigate();
  const isTauri = useIsTauri();

  // file object or string path (desktop uses string path)
  const [file, setFile] = useState<SelectedFile | null>(null);
  const [selectedModelId, setSelectedModelId] = useState('');
  const [preparingStage, setPreparingStage] = useState<PreparingStage>(null);
  const [taskId, setTaskId] = useState<string | null>(null);

  const { handleFile } = useFile();
  const transcribe = useTranscribe();
  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
    error: catalogModelsError,
    refetch: refetchCatalogModels,
  } = api.useQuery('get', '/v1/models');
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const whisperModels = useMemo(
    () => (catalogModels ?? []).filter((model) => model.backend_ids.includes(WHISPER_BACKEND_ID)),
    [catalogModels]
  );

  const selectedModel = useMemo(
    () => whisperModels.find((model) => model.id === selectedModelId),
    [whisperModels, selectedModelId]
  );

  const isBusy =
    Boolean(preparingStage) ||
    transcribe.isPending ||
    loadModelMutation.isPending ||
    downloadModelMutation.isPending;

  useEffect(() => {
    if (whisperModels.length === 0) {
      setSelectedModelId('');
      return;
    }

    const exists = whisperModels.some((model) => model.id === selectedModelId);
    if (!selectedModelId || !exists) {
      setSelectedModelId(whisperModels[0].id);
    }
  }, [whisperModels, selectedModelId]);

  const pendingTaskIdOf = (model: unknown): string | null => {
    if (typeof model !== 'object' || model === null) return null;
    const pendingTaskId = (model as { pending_task_id?: string | null }).pending_task_id;
    if (typeof pendingTaskId !== 'string') return null;
    const trimmed = pendingTaskId.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

  const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

  const extractTaskId = (payload: unknown): string | null => {
    if (typeof payload !== 'object' || payload === null) return null;
    const taskId = (payload as { task_id?: unknown }).task_id;
    if (typeof taskId !== 'string') return null;
    const trimmed = taskId.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;

    while (Date.now() < deadline) {
      const task = (await getTaskMutation.mutateAsync({
        params: {
          path: { id: taskId },
        },
      })) as { status: string; error_msg?: string | null };

      if (task.status === 'succeeded') {
        return;
      }

      if (task.status === 'failed' || task.status === 'cancelled' || task.status === 'interrupted') {
        throw new Error(task.error_msg ?? `Task ${taskId} ended with status: ${task.status}`);
      }

      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
    }

    throw new Error('Model download timed out');
  };

  const refreshCatalogAndFindModel = async (modelId: string) => {
    const refreshed = await refetchCatalogModels();
    const models = refreshed.data ?? [];
    return models.find((model) => model.id === modelId);
  };

  const ensureDownloadedModelPath = async (
    modelId: string
  ): Promise<{ modelPath: string; downloadedNow: boolean }> => {
    let model = whisperModels.find((item) => item.id === modelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId);
    }

    if (!model) {
      throw new Error('Selected model does not exist in catalog');
    }

    if (!model.backend_ids.includes(WHISPER_BACKEND_ID)) {
      throw new Error(`Selected model does not support ${WHISPER_BACKEND_ID}`);
    }

    if (model.local_path) {
      return { modelPath: model.local_path, downloadedNow: false };
    }

    let taskId = pendingTaskIdOf(model);
    if (!taskId) {
      const downloadResponse = await downloadModelMutation.mutateAsync({
        body: {
          backend_id: WHISPER_BACKEND_ID,
          model_id: modelId,
        },
      });
      taskId = extractTaskId(downloadResponse);
    }

    if (!taskId) {
      throw new Error('Failed to start model download task');
    }

    await waitForTaskToFinish(taskId);

    const refreshedModel = await refreshCatalogAndFindModel(modelId);
    if (!refreshedModel?.local_path) {
      throw new Error('Model download completed, but local_path is empty');
    }

    return { modelPath: refreshedModel.local_path, downloadedNow: true };
  };

  const prepareSelectedModel = async (): Promise<string> => {
    if (!selectedModelId) {
      throw new Error('Please select a whisper model first.');
    }

    const model = whisperModels.find((item) => item.id === selectedModelId);
    if (!model) {
      throw new Error('Selected model no longer exists in catalog.');
    }

    const { modelPath, downloadedNow } = await ensureDownloadedModelPath(selectedModelId);

    if (downloadedNow) {
      toast.success(`Downloaded ${model.display_name}`);
    }

    await loadModelMutation.mutateAsync({
      body: {
        backend_id: WHISPER_BACKEND_ID,
        model_path: modelPath,
      },
    });

    return model.display_name;
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    e.preventDefault();
    const selectedFile = await handleFile(e);

    if (selectedFile) {
      setFile(selectedFile);
    }
  };

  const handleTauriFileSelect = async () => {
    const selectedFile = await handleFile();
    if (selectedFile) {
      setFile(selectedFile);
    }
  };

  const handleTranscribe = async () => {
    if (!isTauri) {
      toast.error('Web transcription upload is not implemented yet. Please use the desktop app.');
      return;
    }

    if (!file) {
      toast.error('Please select a file first.');
      return;
    }

    if (!selectedModelId) {
      toast.error('Please select a whisper model first.');
      return;
    }

    try {
      setTaskId(null);
      setPreparingStage('prepare');
      const modelName = await prepareSelectedModel();

      setPreparingStage('transcribe');
      const result = await transcribe.handleTranscribe(file.file);
      setTaskId(result.task_id);

      toast.success('Transcription task created.', {
        description: `Task ID: ${result.task_id} | Model: ${modelName}`,
        action: {
          label: 'View tasks',
          onClick: () => navigate('/task')
        }
      });
    } catch (err: any) {
      toast.error('Failed to create transcription task.', {
        description: err?.message || err?.error || 'Unknown error'
      });
    } finally {
      setPreparingStage(null);
    }
  };

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <div className="text-center space-y-4">
        <h1 className="text-3xl font-bold text-center">Audio Transcription</h1>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          {isTauri
            ? 'Desktop mode: select a local audio/video file path and submit it as a URL path.'
            : 'Web upload is not implemented yet; this will be added later.'}
        </p>
      </div>

      <Card className="max-w-2xl mx-auto">
        <CardHeader>
          <CardTitle>Transcription Setup</CardTitle>
          <CardDescription>
            Choose a whisper model and file. If model files are missing, they will be downloaded and loaded automatically. Worker count comes from Settings configuration.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {catalogModelsError && (
            <Alert variant="destructive">
              <AlertTitle>Model Catalog Error</AlertTitle>
              <AlertDescription>
                {(catalogModelsError as any)?.message || 'Failed to load model catalog. Please check server status.'}
              </AlertDescription>
            </Alert>
          )}

          {transcribe?.isError && (
            <Alert variant="destructive">
              <AlertTitle>Error</AlertTitle>
              <AlertDescription>
                {(transcribe?.error as any)?.error || 'Failed to create transcription task. Please retry.'}
              </AlertDescription>
            </Alert>
          )}

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>Whisper Model</Label>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 px-2 text-xs"
                onClick={() => void refetchCatalogModels()}
                disabled={isBusy || catalogModelsLoading}
              >
                Refresh
              </Button>
            </div>
            <Select
              value={selectedModelId}
              onValueChange={setSelectedModelId}
              disabled={!isTauri || isBusy || catalogModelsLoading || whisperModels.length === 0}
            >
              <SelectTrigger>
                <SelectValue placeholder={catalogModelsLoading ? 'Loading models...' : 'Select whisper model'} />
              </SelectTrigger>
              <SelectContent>
                {whisperModels.length === 0 ? (
                  <div className="px-2 py-1.5 text-sm text-muted-foreground">
                    No whisper models in catalog
                  </div>
                ) : (
                  whisperModels.map((model) => (
                    <SelectItem key={model.id} value={model.id}>
                      {model.display_name}
                    </SelectItem>
                  ))
                )}
              </SelectContent>
            </Select>
            {selectedModel ? (
              <p className="text-xs text-muted-foreground">
                {selectedModel.local_path
                  ? 'Downloaded locally.'
                  : pendingTaskIdOf(selectedModel)
                    ? 'Download task is running. It will be reused automatically.'
                    : 'Not downloaded yet. It will be downloaded automatically before transcription.'}
              </p>
            ) : (
              <p className="text-xs text-muted-foreground">
                {catalogModelsLoading
                  ? 'Loading model catalog...'
                  : 'No whisper model available. Please add one in Settings first.'}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <Label htmlFor={isTauri ? undefined : 'file'}>File</Label>
            {isTauri ? (
              <Button
                type="button"
                variant="outline"
                onClick={() => void handleTauriFileSelect()}
                disabled={isBusy}
              >
                {file ? 'Change File' : 'Choose File'}
              </Button>
            ) : (
              <Input
                id="file"
                type="file"
                accept="audio/*,video/*"
                onChange={handleFileChange}
                disabled={isBusy || !isTauri}
              />
            )}
            {file && (
              <p className="text-sm text-muted-foreground">
                Selected: {file.name}
              </p>
            )}
          </div>

          {isBusy && (
            <div className="flex flex-col items-center space-y-4">
              <Spinner className="h-8 w-8" />
              <p>
                {preparingStage === 'prepare'
                  ? 'Preparing selected model...'
                  : 'Processing transcription request...'}
              </p>
              {taskId && preparingStage !== 'prepare' && (
                <p className="text-xs text-muted-foreground">Task ID: {taskId}</p>
              )}
            </div>
          )}
        </CardContent>
        <CardFooter className="flex justify-end">
          <Button
            onClick={handleTranscribe}
            disabled={!isTauri || !file || !selectedModelId || isBusy}
          >
            {preparingStage === 'prepare'
              ? 'Preparing Model...'
              : preparingStage === 'transcribe' || transcribe?.isPending
                ? 'Processing...'
                : 'Start Transcription'}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}
