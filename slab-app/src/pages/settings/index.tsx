import { useEffect, useMemo, useState } from 'react';
import api, { getErrorMessage } from "@/lib/api";
import type { paths } from "@/lib/api";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui/tabs';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import {
  Label,
} from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Separator } from '@/components/ui/separator';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { toast } from 'sonner';
import {
  AlertCircle,
  CheckCircle2,
  Download,
  Loader2,
  Pencil,
  Plus,
  Trash2,
  XCircle,
} from 'lucide-react';

const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? 'http://localhost:3000';

interface BackendListItem {
  model_type: string;
  backend: string;
  status: string;
}

type ModelCatalogItem =
  paths["/v1/models"]["get"]["responses"][200]["content"]["application/json"][number];

type BusyAction = 'download' | null;
type StatusFilter = 'all' | 'downloaded' | 'pending' | 'not_downloaded';
type ModelStatus = 'downloaded' | 'pending' | 'not_downloaded';

type ModelDraft = {
  display_name: string;
  repo_id: string;
  filename: string;
  backend_ids: string[];
};

const EMPTY_MODEL_DRAFT: ModelDraft = {
  display_name: '',
  repo_id: '',
  filename: '',
  backend_ids: [],
};

const MODEL_AUTO_UNLOAD_ENABLED_KEY = 'model_auto_unload_enabled';
const MODEL_AUTO_UNLOAD_IDLE_MINUTES_KEY = 'model_auto_unload_idle_minutes';
const CHAT_MODEL_PROVIDERS_KEY = 'chat_model_providers';

const DIFFUSION_VAE_PATH_KEY = 'diffusion_vae_path';
const DIFFUSION_TAESD_PATH_KEY = 'diffusion_taesd_path';
const DIFFUSION_LORA_MODEL_DIR_KEY = 'diffusion_lora_model_dir';
const DIFFUSION_CLIP_L_PATH_KEY = 'diffusion_clip_l_path';
const DIFFUSION_CLIP_G_PATH_KEY = 'diffusion_clip_g_path';
const DIFFUSION_T5XXL_PATH_KEY = 'diffusion_t5xxl_path';
const DIFFUSION_FLASH_ATTN_KEY = 'diffusion_flash_attn';
const DIFFUSION_KEEP_VAE_ON_CPU_KEY = 'diffusion_keep_vae_on_cpu';
const DIFFUSION_KEEP_CLIP_ON_CPU_KEY = 'diffusion_keep_clip_on_cpu';
const DIFFUSION_OFFLOAD_PARAMS_KEY = 'diffusion_offload_params_to_cpu';
const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;


function parseConfigBool(value?: string | null) {
  if (!value) return false;
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase());
}

function formatDate(value?: string | null) {
  if (!value) return 'Never';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return parsed.toLocaleString();
}

export default function Settings() {
  const [selectedConfigKey, setSelectedConfigKey] = useState<string | null>(null);
  const [configName, setConfigName] = useState<string>('');
  const [configValue, setConfigValue] = useState<string>('');
  const [downloadingBackend, setDownloadingBackend] = useState<string | null>(null);
  const [autoUnloadEnabled, setAutoUnloadEnabled] = useState(false);
  const [autoUnloadMinutes, setAutoUnloadMinutes] = useState('10');
  const [isSavingAutoUnload, setIsSavingAutoUnload] = useState(false);
  const [chatProvidersRaw, setChatProvidersRaw] = useState('[]');
  const [isSavingChatProviders, setIsSavingChatProviders] = useState(false);

  // Diffusion global settings
  const [diffusionVaePath, setDiffusionVaePath] = useState('');
  const [diffusionTaesdPath, setDiffusionTaesdPath] = useState('');
  const [diffusionLoraModelDir, setDiffusionLoraModelDir] = useState('');
  const [diffusionClipLPath, setDiffusionClipLPath] = useState('');
  const [diffusionClipGPath, setDiffusionClipGPath] = useState('');
  const [diffusionT5xxlPath, setDiffusionT5xxlPath] = useState('');
  const [diffusionFlashAttn, setDiffusionFlashAttn] = useState(false);
  const [diffusionKeepVaeOnCpu, setDiffusionKeepVaeOnCpu] = useState(false);
  const [diffusionKeepClipOnCpu, setDiffusionKeepClipOnCpu] = useState(false);
  const [diffusionOffloadParams, setDiffusionOffloadParams] = useState(false);
  const [isSavingDiffusion, setIsSavingDiffusion] = useState(false);

  const [isModelDialogOpen, setIsModelDialogOpen] = useState(false);
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [modelDraft, setModelDraft] = useState<ModelDraft>(EMPTY_MODEL_DRAFT);
  const [deletingModelId, setDeletingModelId] = useState<string | null>(null);
  const [searchKeyword, setSearchKeyword] = useState('');
  const [backendFilter, setBackendFilter] = useState('all');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [selectedModelId, setSelectedModelId] = useState('');
  const [selectedBackendId, setSelectedBackendId] = useState('');
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [busyModelId, setBusyModelId] = useState<string | null>(null);

  // API calls using react-query
  const { data: configs, error: configsError, isLoading: configsLoading, refetch: refetchConfigs } = api.useQuery('get', '/v1/config');
  const { data: backends, error: backendsError, isLoading: backendsLoading } = api.useQuery('get', '/v1/backends');
  const { data: models, error: modelsError, isLoading: modelsLoading, refetch: refetchModels } = api.useQuery('get', '/v1/models');

  // Mutations
  const createModelMutation = api.useMutation('post', '/v1/models');
  const updateModelMutation = api.useMutation('put', '/v1/models/{id}');
  const deleteModelMutation = api.useMutation('delete', '/v1/models/{id}');
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const backendList =
    typeof backends === 'object' &&
    backends !== null &&
    Array.isArray((backends as { backends?: unknown }).backends)
      ? ((backends as { backends: BackendListItem[] }).backends ?? [])
      : [];

  const availableBackendIds = useMemo(() => {
    const fromApi = backendList.map((backend) => backend.backend).filter((backend) => backend.startsWith('ggml.'));
    const unique = Array.from(new Set(fromApi));
    if (unique.length > 0) return unique;
    return ['ggml.llama', 'ggml.whisper', 'ggml.diffusion'];
  }, [backendList]);

  const modelList: ModelCatalogItem[] = Array.isArray(models) ? models : [];
  const installedModelCount = modelList.filter((model) => Boolean(model.local_path)).length;
  const configEntries = Array.isArray(configs) ? configs : [];
  const configValueByKey = useMemo(
    () => new Map(configEntries.map((entry) => [entry.key, entry.value])),
    [configEntries]
  );

  const isSavingModel = createModelMutation.isPending || updateModelMutation.isPending;
  const isBusy = busyAction !== null;

  const pendingTaskIdOf = (model: ModelCatalogItem): string | null => {
    const pendingTaskId = model.pending_task_id;
    if (typeof pendingTaskId !== 'string') return null;
    const trimmed = pendingTaskId.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

  const statusOfModel = (model: ModelCatalogItem): ModelStatus => {
    if (model.local_path) return 'downloaded';
    if (pendingTaskIdOf(model)) return 'pending';
    return 'not_downloaded';
  };

  const selectedModel = useMemo(
    () => modelList.find((model) => model.id === selectedModelId),
    [modelList, selectedModelId]
  );

  const modelBackendOptions = useMemo(() => {
    const unique = new Set<string>();
    for (const model of modelList) {
      for (const backend of model.backend_ids) {
        unique.add(backend);
      }
    }
    return Array.from(unique).sort();
  }, [modelList]);

  const filteredModels = useMemo(() => {
    const keyword = searchKeyword.trim().toLowerCase();
    return modelList.filter((model) => {
      if (backendFilter !== 'all' && !model.backend_ids.includes(backendFilter)) {
        return false;
      }

      const status = statusOfModel(model);
      if (statusFilter !== 'all' && status !== statusFilter) {
        return false;
      }

      if (!keyword) {
        return true;
      }

      const haystack = [
        model.id,
        model.display_name,
        model.repo_id,
        model.filename,
        model.local_path ?? '',
      ]
        .join(' ')
        .toLowerCase();

      return haystack.includes(keyword);
    });
  }, [backendFilter, modelList, searchKeyword, statusFilter]);

  useEffect(() => {
    if (modelList.length === 0) {
      setSelectedModelId('');
      return;
    }
    const exists = modelList.some((model) => model.id === selectedModelId);
    if (!selectedModelId || !exists) {
      setSelectedModelId(modelList[0].id);
    }
  }, [modelList, selectedModelId]);

  useEffect(() => {
    if (!selectedModel) {
      setSelectedBackendId('');
      return;
    }
    const compatible = selectedModel.backend_ids.includes(selectedBackendId);
    if (!selectedBackendId || !compatible) {
      setSelectedBackendId(selectedModel.backend_ids[0] ?? '');
    }
  }, [selectedModel, selectedBackendId]);

  useEffect(() => {
    setAutoUnloadEnabled(parseConfigBool(configValueByKey.get(MODEL_AUTO_UNLOAD_ENABLED_KEY)));
    setAutoUnloadMinutes(configValueByKey.get(MODEL_AUTO_UNLOAD_IDLE_MINUTES_KEY)?.trim() || '10');
    setChatProvidersRaw(configValueByKey.get(CHAT_MODEL_PROVIDERS_KEY)?.trim() || '[]');
    setDiffusionVaePath(configValueByKey.get(DIFFUSION_VAE_PATH_KEY) ?? '');
    setDiffusionTaesdPath(configValueByKey.get(DIFFUSION_TAESD_PATH_KEY) ?? '');
    setDiffusionLoraModelDir(configValueByKey.get(DIFFUSION_LORA_MODEL_DIR_KEY) ?? '');
    setDiffusionClipLPath(configValueByKey.get(DIFFUSION_CLIP_L_PATH_KEY) ?? '');
    setDiffusionClipGPath(configValueByKey.get(DIFFUSION_CLIP_G_PATH_KEY) ?? '');
    setDiffusionT5xxlPath(configValueByKey.get(DIFFUSION_T5XXL_PATH_KEY) ?? '');
    setDiffusionFlashAttn(parseConfigBool(configValueByKey.get(DIFFUSION_FLASH_ATTN_KEY)));
    setDiffusionKeepVaeOnCpu(parseConfigBool(configValueByKey.get(DIFFUSION_KEEP_VAE_ON_CPU_KEY)));
    setDiffusionKeepClipOnCpu(parseConfigBool(configValueByKey.get(DIFFUSION_KEEP_CLIP_ON_CPU_KEY)));
    setDiffusionOffloadParams(parseConfigBool(configValueByKey.get(DIFFUSION_OFFLOAD_PARAMS_KEY)));

  }, [configValueByKey]);

  const resetModelDialog = () => {
    setIsModelDialogOpen(false);
    setEditingModelId(null);
    setModelDraft(EMPTY_MODEL_DRAFT);
  };

  const openCreateModelDialog = () => {
    setEditingModelId(null);
    setModelDraft({
      ...EMPTY_MODEL_DRAFT,
      backend_ids: availableBackendIds.length > 0 ? [availableBackendIds[0]] : [],
    });
    setIsModelDialogOpen(true);
  };

  const openEditModelDialog = (model: ModelCatalogItem) => {
    setEditingModelId(model.id);
    setModelDraft({
      display_name: model.display_name,
      repo_id: model.repo_id,
      filename: model.filename,
      backend_ids: model.backend_ids,
    });
    setIsModelDialogOpen(true);
  };

  const setModelField = (field: keyof ModelDraft, value: string) => {
    setModelDraft((prev) => ({
      ...prev,
      [field]: value,
    }));
  };

  const toggleBackendId = (backendId: string, checked: boolean) => {
    setModelDraft((prev) => {
      if (checked) {
        if (prev.backend_ids.includes(backendId)) return prev;
        return { ...prev, backend_ids: [...prev.backend_ids, backendId] };
      }
      return {
        ...prev,
        backend_ids: prev.backend_ids.filter((id) => id !== backendId),
      };
    });
  };

  const saveModel = async () => {
    const display_name = modelDraft.display_name.trim();
    const repo_id = modelDraft.repo_id.trim();
    const filename = modelDraft.filename.trim();
    const backend_ids = modelDraft.backend_ids;

    if (!display_name || !repo_id || !filename) {
      toast.error('Display name, repository ID, and filename are required.');
      return;
    }
    if (backend_ids.length === 0) {
      toast.error('Select at least one backend.');
      return;
    }

    try {
      if (editingModelId) {
        await updateModelMutation.mutateAsync({
          params: {
            path: { id: editingModelId },
          },
          body: {
            display_name,
            repo_id,
            filename,
            backend_ids,
          },
        });
        toast.success('Model updated');
      } else {
        await createModelMutation.mutateAsync({
          body: {
            display_name,
            repo_id,
            filename,
            backend_ids,
          },
        });
        toast.success('Model added to catalog');
      }
      await refetchModels();
      resetModelDialog();
    } catch (error) {
      toast.error(getErrorMessage(error));
    }
  };

  const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

  const extractTaskId = (payload: unknown): string | null => {
    if (typeof payload !== 'object' || payload === null) return null;
    const taskId =
      (payload as { operation_id?: unknown }).operation_id ??
      (payload as { task_id?: unknown }).task_id;
    if (typeof taskId !== 'string') return null;
    const trimmed = taskId.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

  const putConfigValue = async (key: string, body: { name?: string; value: string }) => {
    const response = await fetch(`${API_BASE_URL}/v1/config/${encodeURIComponent(key)}`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      const detail = await response.text();
      throw new Error(detail || `HTTP ${response.status}`);
    }

    return response.json();
  };

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;
    while (Date.now() < deadline) {
      const task = await getTaskMutation.mutateAsync({
        params: {
          path: { id: taskId },
        },
      });

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
    const refreshed = await refetchModels();
    const entries = refreshed.data ?? [];
    return entries.find((model) => model.id === modelId);
  };

  const ensureDownloadedModelPath = async (modelId: string, backendId: string): Promise<string> => {
    let model = modelList.find((item) => item.id === modelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId);
    }
    if (!model) {
      throw new Error('Selected model does not exist in catalog');
    }

    if (model.local_path) {
      return model.local_path;
    }

    let taskId = pendingTaskIdOf(model);
    if (!taskId) {
      const downloadResponse = await downloadModelMutation.mutateAsync({
        body: {
          backend_id: backendId,
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
    return refreshedModel.local_path;
  };

  const chooseBackendForModel = (model: { backend_ids: string[] }): string => {
    if (selectedBackendId && model.backend_ids.includes(selectedBackendId)) {
      return selectedBackendId;
    }
    return model.backend_ids[0] ?? '';
  };

  const runDownloadOnly = async (modelId: string, backendId: string) => {
    const model = modelList.find((item) => item.id === modelId);
    if (!model) {
      toast.error('Model no longer exists');
      return;
    }
    if (!backendId) {
      toast.error('No available backend for this model');
      return;
    }
    if (!model.backend_ids.includes(backendId)) {
      toast.error('Selected backend is not supported by this model');
      return;
    }

    setBusyAction('download');
    setBusyModelId(modelId);
    try {
      if (model.local_path) {
        toast.success(`${model.display_name} is already downloaded`);
        return;
      }

      await ensureDownloadedModelPath(modelId, backendId);
      toast.success(`Downloaded ${model.display_name}`);
      await refetchModels();
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setBusyAction(null);
      setBusyModelId(null);
    }
  };

  const handleRowDownload = async (modelId: string) => {
    const model = modelList.find((item) => item.id === modelId);
    if (!model) return;

    const backendId = chooseBackendForModel(model);
    setSelectedModelId(modelId);
    setSelectedBackendId(backendId);
    await runDownloadOnly(modelId, backendId);
  };

  const statusBadge = (status: ModelStatus) => {
    if (status === 'downloaded') return <Badge className="bg-green-100 text-green-800 hover:bg-green-100 border border-green-200">Downloaded</Badge>;
    if (status === 'pending') return <Badge variant="secondary">Downloading</Badge>;
    return <Badge variant="outline">Not Downloaded</Badge>;
  };

  // Function to update config value
  const updateConfig = async (key: string, value: string, name?: string) => {
    try {
      await putConfigValue(key, { name, value });
      toast.success('Configuration updated successfully');
      refetchConfigs();
    } catch (error) {
      toast.error(getErrorMessage(error));
    }
  };

  // Function to get backend status
  const getBackendStatus = async (backendId: string) => {
    try {
      const response = await fetch(
        `${API_BASE_URL}/v1/backends/status?backend_id=${encodeURIComponent(backendId)}`
      );
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const status = (await response.json()) as { status?: string };
      toast.success(`Backend Status: ${status?.status}`);
    } catch (error) {
      toast.error(getErrorMessage(error));
    }
  };

  // Function to download backend
  const downloadBackend = async (backendId: string) => {
    setDownloadingBackend(backendId);
    try {
      toast.warning(
        `Backend ${backendId} download requires backend_id and target_dir. Use /v1/backends/download with payload like {"backend_id":"ggml.llama","target_dir":"C:\\\\slab\\\\llama"}.`
      );
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setDownloadingBackend(null);
    }
  };

  const deleteModel = async (id: string) => {
    setDeletingModelId(id);
    try {
      await deleteModelMutation.mutateAsync({
        params: {
          path: { id },
        },
      });
      toast.success('Model removed from catalog');
      await refetchModels();
      if (editingModelId === id) {
        resetModelDialog();
      }
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setDeletingModelId(null);
    }
  };

  const saveModelAutoUnload = async () => {
    if (autoUnloadEnabled) {
      const parsed = Number.parseInt(autoUnloadMinutes, 10);
      if (!Number.isFinite(parsed) || parsed < 1) {
        toast.error('Idle minutes must be an integer greater than or equal to 1.');
        return;
      }
    }

    setIsSavingAutoUnload(true);
    try {
      await putConfigValue(MODEL_AUTO_UNLOAD_ENABLED_KEY, {
        name: 'Model Auto Unload Enabled',
        value: autoUnloadEnabled ? 'true' : 'false',
      });

      await putConfigValue(MODEL_AUTO_UNLOAD_IDLE_MINUTES_KEY, {
        name: 'Model Auto Unload Idle Minutes',
        value: autoUnloadMinutes.trim() || '10',
      });

      toast.success('Model auto-unload configuration saved');
      await refetchConfigs();
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setIsSavingAutoUnload(false);
    }
  };

  const saveChatProviders = async () => {
    const raw = chatProvidersRaw.trim() || '[]';

    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) {
        toast.error('Chat model providers must be a JSON array.');
        return;
      }
    } catch (error: any) {
      toast.error('Invalid JSON for chat model providers.', {
        description: error?.message || 'Unknown parse error',
      });
      return;
    }

    setIsSavingChatProviders(true);
    try {
      await putConfigValue(CHAT_MODEL_PROVIDERS_KEY, {
        name: 'Chat Model Providers',
        value: raw,
      });
      toast.success('Chat model providers saved');
      await refetchConfigs();
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setIsSavingChatProviders(false);
    }
  };

  const saveDiffusionSettings = async () => {
    setIsSavingDiffusion(true);
    const entries: Array<[string, string]> = [
      [DIFFUSION_VAE_PATH_KEY, diffusionVaePath],
      [DIFFUSION_TAESD_PATH_KEY, diffusionTaesdPath],
      [DIFFUSION_LORA_MODEL_DIR_KEY, diffusionLoraModelDir],
      [DIFFUSION_CLIP_L_PATH_KEY, diffusionClipLPath],
      [DIFFUSION_CLIP_G_PATH_KEY, diffusionClipGPath],
      [DIFFUSION_T5XXL_PATH_KEY, diffusionT5xxlPath],
      [DIFFUSION_FLASH_ATTN_KEY, diffusionFlashAttn ? '1' : '0'],
      [DIFFUSION_KEEP_VAE_ON_CPU_KEY, diffusionKeepVaeOnCpu ? '1' : '0'],
      [DIFFUSION_KEEP_CLIP_ON_CPU_KEY, diffusionKeepClipOnCpu ? '1' : '0'],
      [DIFFUSION_OFFLOAD_PARAMS_KEY, diffusionOffloadParams ? '1' : '0'],
    ];
    try {
      await Promise.all(entries.map(([key, value]) =>
        putConfigValue(key, { name: key, value })
      ));
      toast.success('Diffusion settings saved');
      await refetchConfigs();
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setIsSavingDiffusion(false);
    }
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="container mx-auto space-y-8 px-4 py-8">
      <h1 className="text-3xl font-bold">Settings</h1>

      <Tabs defaultValue="config">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="config">Configuration</TabsTrigger>
          <TabsTrigger value="diffusion">Diffusion</TabsTrigger>
          <TabsTrigger value="backends">Backends</TabsTrigger>
        </TabsList>

        <TabsContent value="models" className="mt-6">
          <Card>
            <CardHeader className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
              <div>
                <CardTitle>Model Catalog</CardTitle>
                <CardDescription>
                  Manage model catalog entries and downloads in one place.
                </CardDescription>
              </div>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  onClick={() => refetchModels()}
                  disabled={modelsLoading || isBusy}
                >
                  {modelsLoading && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                  Refresh
                </Button>
                <Button onClick={openCreateModelDialog} disabled={isBusy}>
                  <Plus className="h-4 w-4 mr-2" />
                  Add Model
                </Button>
              </div>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                <div className="rounded-lg border p-4">
                  <p className="text-xs text-muted-foreground">Catalog Models</p>
                  <p className="mt-1 text-2xl font-semibold">{modelList.length}</p>
                </div>
                <div className="rounded-lg border p-4">
                  <p className="text-xs text-muted-foreground">Installed Models</p>
                  <p className="mt-1 text-2xl font-semibold">{installedModelCount}</p>
                </div>
                <div className="rounded-lg border p-4">
                  <p className="text-xs text-muted-foreground">Not Downloaded</p>
                  <p className="mt-1 text-2xl font-semibold">{modelList.length - installedModelCount}</p>
                </div>
              </div>

              <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                <Input
                  value={searchKeyword}
                  onChange={(event) => setSearchKeyword(event.target.value)}
                  placeholder="Search model / repo / file"
                  disabled={isBusy}
                />

                <Select value={backendFilter} onValueChange={setBackendFilter} disabled={isBusy}>
                  <SelectTrigger>
                    <SelectValue placeholder="Filter by backend" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">All backends</SelectItem>
                    {modelBackendOptions.map((backendId) => (
                      <SelectItem key={backendId} value={backendId}>
                        {backendId}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <Select value={statusFilter} onValueChange={(value) => setStatusFilter(value as StatusFilter)} disabled={isBusy}>
                  <SelectTrigger>
                    <SelectValue placeholder="Filter by status" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">All status</SelectItem>
                    <SelectItem value="downloaded">Downloaded</SelectItem>
                    <SelectItem value="pending">Downloading</SelectItem>
                    <SelectItem value="not_downloaded">Not downloaded</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              {modelsLoading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin" />
                  <span className="ml-2">Loading model catalog...</span>
                </div>
              ) : modelsError ? (
                <div className="text-red-500">Error loading model catalog</div>
              ) : (
                <div className="rounded-md border">
                  <Table>
                    <TableCaption>Catalog entries and download status</TableCaption>
                    <TableHeader>
                      <TableRow>
                        <TableHead className="w-[220px]">Model</TableHead>
                        <TableHead className="w-[320px]">Repo / File</TableHead>
                        <TableHead className="w-[220px]">Backends</TableHead>
                        <TableHead className="w-[140px]">Status</TableHead>
                        <TableHead>Local Path</TableHead>
                        <TableHead className="w-[180px]">Last Download</TableHead>
                        <TableHead className="sticky right-0 z-20 w-[320px] border-l bg-background text-right">Actions</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {filteredModels.length > 0 ? (
                        filteredModels.map((model) => {
                          const rowBackend = chooseBackendForModel(model);
                          const rowBusy = busyModelId === model.id;
                          const isSelected = selectedModelId === model.id;

                          return (
                            <TableRow
                              key={model.id}
                              className={isSelected ? 'group bg-muted/40' : 'group'}
                              onClick={() => {
                                setSelectedModelId(model.id);
                                setSelectedBackendId(rowBackend);
                              }}
                            >
                              <TableCell>
                                <div className="space-y-1">
                                  <p className="font-medium">{model.display_name}</p>
                                  <p className="truncate font-mono text-xs text-muted-foreground">{model.id}</p>
                                </div>
                              </TableCell>
                              <TableCell>
                                <div className="space-y-1 text-xs text-muted-foreground">
                                  <p className="truncate">{model.repo_id}</p>
                                  <p className="truncate">{model.filename}</p>
                                </div>
                              </TableCell>
                              <TableCell>
                                <div className="flex flex-wrap gap-1">
                                  {model.backend_ids.map((backendId) => (
                                    <Badge key={backendId} variant={backendId === rowBackend ? 'default' : 'outline'}>
                                      {backendId}
                                    </Badge>
                                  ))}
                                </div>
                              </TableCell>
                              <TableCell>{statusBadge(statusOfModel(model))}</TableCell>
                              <TableCell className="max-w-[320px] truncate text-xs text-muted-foreground">
                                {model.local_path ?? '-'}
                              </TableCell>
                              <TableCell>{formatDate(model.last_downloaded_at)}</TableCell>
                              <TableCell className={`sticky right-0 z-10 border-l group-hover:bg-muted/50 ${isSelected ? 'bg-muted/40' : 'bg-background'}`}>
                                <div className="flex items-center justify-end gap-2">
                                  <Button
                                    variant="outline"
                                    size="sm"
                                    onClick={(event) => {
                                      event.stopPropagation();
                                      void handleRowDownload(model.id);
                                    }}
                                    disabled={isBusy || deletingModelId === model.id}
                                  >
                                    {rowBusy && busyAction === 'download' ? (
                                      <>
                                        <Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />
                                        Downloading...
                                      </>
                                    ) : (
                                      <>
                                        <Download className="h-3.5 w-3.5 mr-1" />
                                        Download
                                      </>
                                    )}
                                  </Button>
                                  <Button
                                    variant="outline"
                                    size="sm"
                                    onClick={(event) => {
                                      event.stopPropagation();
                                      openEditModelDialog(model);
                                    }}
                                    disabled={isBusy}
                                  >
                                    <Pencil className="h-3.5 w-3.5 mr-1" />
                                    Edit
                                  </Button>
                                  <AlertDialog>
                                    <AlertDialogTrigger asChild>
                                      <Button
                                        variant="destructive"
                                        size="sm"
                                        disabled={deletingModelId === model.id || isBusy}
                                        onClick={(event) => event.stopPropagation()}
                                      >
                                        {deletingModelId === model.id ? (
                                          <Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />
                                        ) : (
                                          <Trash2 className="h-3.5 w-3.5 mr-1" />
                                        )}
                                        Delete
                                      </Button>
                                    </AlertDialogTrigger>
                                    <AlertDialogContent size="sm">
                                      <AlertDialogHeader>
                                        <AlertDialogTitle>Delete model entry?</AlertDialogTitle>
                                        <AlertDialogDescription>
                                          This will remove <strong>{model.display_name}</strong> from the model catalog.
                                        </AlertDialogDescription>
                                      </AlertDialogHeader>
                                      <AlertDialogFooter>
                                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                                        <AlertDialogAction
                                          variant="destructive"
                                          onClick={() => void deleteModel(model.id)}
                                        >
                                          Delete
                                        </AlertDialogAction>
                                      </AlertDialogFooter>
                                    </AlertDialogContent>
                                  </AlertDialog>
                                </div>
                              </TableCell>
                            </TableRow>
                          );
                        })
                      ) : (
                        <TableRow>
                          <TableCell colSpan={7} className="py-8 text-center">
                            {modelList.length === 0 ? (
                              <div className="space-y-2">
                                <p className="font-medium">No model entries yet</p>
                                <p className="text-sm text-muted-foreground">
                                  Add your first model to make it available for downloads and workflows.
                                </p>
                                <Button onClick={openCreateModelDialog} size="sm" disabled={isBusy}>
                                  <Plus className="h-4 w-4 mr-2" />
                                  Add First Model
                                </Button>
                              </div>
                            ) : (
                              <p className="text-muted-foreground">No models matched the filters</p>
                            )}
                          </TableCell>
                        </TableRow>
                      )}
                    </TableBody>
                  </Table>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="config" className="mt-6">
          <Card>
            <CardHeader>
              <CardTitle>Configuration Management</CardTitle>
              <CardDescription>
                View and update configuration settings. Global backend keys: `llama_num_workers`, `whisper_num_workers`, `diffusion_num_workers`, `llama_context_length`, `model_auto_unload_enabled`, `model_auto_unload_idle_minutes`.
              </CardDescription>
            </CardHeader>
            <CardContent>
              {configsLoading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin" />
                  <span className="ml-2">Loading configurations...</span>
                </div>
              ) : configsError ? (
                <div className="text-red-500">Error loading configurations</div>
              ) : (
                <div className="space-y-6">
                  <Card className="border-dashed">
                    <CardHeader>
                      <CardTitle className="text-base">Model Auto Unload</CardTitle>
                      <CardDescription>
                        Track active model references and unload when idle for the configured minutes.
                      </CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-4">
                      <div className="flex items-center justify-between rounded-md border p-3">
                        <div>
                          <Label htmlFor="auto-unload-enabled">Enable auto unload</Label>
                          <p className="text-xs text-muted-foreground">
                            When enabled, idle models will be unloaded to free memory.
                          </p>
                        </div>
                        <Checkbox
                          id="auto-unload-enabled"
                          checked={autoUnloadEnabled}
                          onCheckedChange={(checked) => setAutoUnloadEnabled(Boolean(checked))}
                        />
                      </div>

                      <div className="grid gap-2 sm:max-w-[240px]">
                        <Label htmlFor="auto-unload-minutes">Idle timeout (minutes)</Label>
                        <Input
                          id="auto-unload-minutes"
                          type="number"
                          min={1}
                          step={1}
                          value={autoUnloadMinutes}
                          onChange={(e) => setAutoUnloadMinutes(e.target.value)}
                          placeholder="10"
                        />
                      </div>

                      <div>
                        <Button onClick={() => void saveModelAutoUnload()} disabled={isSavingAutoUnload}>
                          {isSavingAutoUnload && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                          Save Auto Unload
                        </Button>
                      </div>
                    </CardContent>
                  </Card>

                  <Card className="border-dashed">
                    <CardHeader>
                      <CardTitle className="text-base">Chat Model Providers</CardTitle>
                      <CardDescription>
                        Configure cloud providers used by Chat model selection. Chat reads this value from
                        <code className="mx-1">chat_model_providers</code>. Use
                        <code className="mx-1">api_key</code> for a literal key, or
                        <code className="mx-1">api_key_env</code> for an environment variable name.
                      </CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-4">
                      <div className="grid gap-2">
                        <Label htmlFor="chat-model-providers-json">Providers JSON</Label>
                        <Textarea
                          id="chat-model-providers-json"
                          className="min-h-[220px] font-mono text-xs"
                          value={chatProvidersRaw}
                          onChange={(e) => setChatProvidersRaw(e.target.value)}
                          placeholder='[]'
                        />
                      </div>

                      <div className="rounded-md border bg-muted/40 p-3">
                        <p className="mb-2 text-xs font-medium">Example</p>
                        <pre className="overflow-auto text-[11px] leading-relaxed text-muted-foreground">
{`[
  {
    "id": "openai-main",
    "name": "OpenAI",
    "api_base": "https://api.openai.com/v1",
    "api_key": "sk-your-api-key",
    "models": [
      { "id": "gpt-4.1-mini", "display_name": "GPT-4.1 Mini" },
      { "id": "gpt-4.1", "display_name": "GPT-4.1" }
    ]
  }
]`}
                        </pre>
                      </div>

                      <div>
                        <Button onClick={() => void saveChatProviders()} disabled={isSavingChatProviders}>
                          {isSavingChatProviders && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                          Save Chat Providers
                        </Button>
                      </div>
                    </CardContent>
                  </Card>

                  <Table>
                    <TableCaption>List of configuration entries</TableCaption>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Key</TableHead>
                        <TableHead>Value</TableHead>
                        <TableHead>Name</TableHead>
                        <TableHead>Actions</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {configEntries.length > 0 ? (
                        configEntries.map((config) => (
                          <TableRow key={config.key}>
                            <TableCell className="font-medium">{config.key}</TableCell>
                            <TableCell>{config.value}</TableCell>
                            <TableCell>{config.name}</TableCell>
                            <TableCell>
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => {
                                  setSelectedConfigKey(config.key);
                                  setConfigName(config.name);
                                  setConfigValue(config.value);
                                }}
                              >
                                Edit
                              </Button>
                            </TableCell>
                          </TableRow>
                        ))
                      ) : (
                        <TableRow>
                          <TableCell colSpan={4} className="text-center py-4">
                            No configuration entries found
                          </TableCell>
                        </TableRow>
                      )}
                    </TableBody>
                  </Table>
                </div>
              )}

              {selectedConfigKey && (
                <Card className="mt-6">
                  <CardHeader>
                    <CardTitle>Edit Configuration: {selectedConfigKey}</CardTitle>
                  </CardHeader>
                  <CardContent>
                    <form
                      onSubmit={(e) => {
                        e.preventDefault();
                        updateConfig(selectedConfigKey, configValue, configName);
                        setSelectedConfigKey(null);
                      }}
                      className="space-y-4"
                    >
                      <div className="grid gap-2">
                        <Label>Name</Label>
                        <Input
                          value={configName}
                          onChange={(e) => setConfigName(e.target.value)}
                          placeholder="Human-readable configuration name"
                        />
                      </div>
                      <div className="grid gap-2">
                        <Label>Value</Label>
                        <Input
                          value={configValue}
                          onChange={(e) => setConfigValue(e.target.value)}
                          placeholder="Enter new value"
                        />
                      </div>
                      <div className="flex gap-2">
                        <Button type="submit">Save</Button>
                        <Button
                          variant="outline"
                          onClick={() => {
                            setSelectedConfigKey(null);
                            setConfigName('');
                          }}
                        >
                          Cancel
                        </Button>
                      </div>
                    </form>
                  </CardContent>
                </Card>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="diffusion" className="mt-6">
          <Card>
            <CardHeader>
              <CardTitle>Diffusion Settings</CardTitle>
              <CardDescription>
                Global parameters applied when loading a diffusion model.
                Changes take effect on the next model load.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="grid gap-4 sm:grid-cols-2">
                <div className="space-y-1.5">
                  <Label htmlFor="vae-path">VAE Path</Label>
                  <Input
                    id="vae-path"
                    placeholder="/models/vae.safetensors"
                    value={diffusionVaePath}
                    onChange={(e) => setDiffusionVaePath(e.target.value)}
                  />
                  <p className="text-xs text-muted-foreground">Optional external VAE model path.</p>
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="taesd-path">TAESD Path</Label>
                  <Input
                    id="taesd-path"
                    placeholder="/models/taesd.safetensors"
                    value={diffusionTaesdPath}
                    onChange={(e) => setDiffusionTaesdPath(e.target.value)}
                  />
                  <p className="text-xs text-muted-foreground">Tiny AutoEncoder for fast decoding.</p>
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="lora-dir">LoRA Model Directory</Label>
                  <Input
                    id="lora-dir"
                    placeholder="/models/loras"
                    value={diffusionLoraModelDir}
                    onChange={(e) => setDiffusionLoraModelDir(e.target.value)}
                  />
                  <p className="text-xs text-muted-foreground">Directory containing LoRA .safetensors files.</p>
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="clip-l">CLIP-L Path</Label>
                  <Input
                    id="clip-l"
                    placeholder="/models/clip_l.safetensors"
                    value={diffusionClipLPath}
                    onChange={(e) => setDiffusionClipLPath(e.target.value)}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="clip-g">CLIP-G Path</Label>
                  <Input
                    id="clip-g"
                    placeholder="/models/clip_g.safetensors"
                    value={diffusionClipGPath}
                    onChange={(e) => setDiffusionClipGPath(e.target.value)}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="t5xxl">T5XXL Path</Label>
                  <Input
                    id="t5xxl"
                    placeholder="/models/t5xxl_fp16.safetensors"
                    value={diffusionT5xxlPath}
                    onChange={(e) => setDiffusionT5xxlPath(e.target.value)}
                  />
                </div>
              </div>
              <Separator />
              <div className="space-y-4">
                <h4 className="font-medium text-sm">Performance Options</h4>
                <div className="grid gap-4 sm:grid-cols-2">
                  <div className="flex items-center justify-between rounded-lg border p-3">
                    <div>
                      <Label htmlFor="flash-attn">Flash Attention</Label>
                      <p className="text-xs text-muted-foreground mt-0.5">Faster attention computation (requires compatible GPU).</p>
                    </div>
                    <Switch
                      id="flash-attn"
                      checked={diffusionFlashAttn}
                      onCheckedChange={setDiffusionFlashAttn}
                    />
                  </div>
                  <div className="flex items-center justify-between rounded-lg border p-3">
                    <div>
                      <Label htmlFor="keep-vae-cpu">Keep VAE on CPU</Label>
                      <p className="text-xs text-muted-foreground mt-0.5">Reduce VRAM usage by keeping VAE on CPU.</p>
                    </div>
                    <Switch
                      id="keep-vae-cpu"
                      checked={diffusionKeepVaeOnCpu}
                      onCheckedChange={setDiffusionKeepVaeOnCpu}
                    />
                  </div>
                  <div className="flex items-center justify-between rounded-lg border p-3">
                    <div>
                      <Label htmlFor="keep-clip-cpu">Keep CLIP on CPU</Label>
                      <p className="text-xs text-muted-foreground mt-0.5">Reduce VRAM usage by keeping CLIP on CPU.</p>
                    </div>
                    <Switch
                      id="keep-clip-cpu"
                      checked={diffusionKeepClipOnCpu}
                      onCheckedChange={setDiffusionKeepClipOnCpu}
                    />
                  </div>
                  <div className="flex items-center justify-between rounded-lg border p-3">
                    <div>
                      <Label htmlFor="offload-params">Offload Params to CPU</Label>
                      <p className="text-xs text-muted-foreground mt-0.5">Reduce VRAM by offloading model params to RAM.</p>
                    </div>
                    <Switch
                      id="offload-params"
                      checked={diffusionOffloadParams}
                      onCheckedChange={setDiffusionOffloadParams}
                    />
                  </div>
                </div>
              </div>
              <Button onClick={() => void saveDiffusionSettings()} disabled={isSavingDiffusion}>
                {isSavingDiffusion && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                Save Diffusion Settings
              </Button>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="backends" className="mt-6">
          <Card>
            <CardHeader>
              <CardTitle>Backend Management</CardTitle>
              <CardDescription>View and manage backend services</CardDescription>
            </CardHeader>
            <CardContent>
              {backendsLoading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin" />
                  <span className="ml-2">Loading backends...</span>
                </div>
              ) : backendsError ? (
                <div className="text-red-500">Error loading backends</div>
              ) : (
                <Table>
                  <TableCaption>List of registered backends</TableCaption>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Backend</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {backendList.length > 0 ? (
                      backendList.map((backend) => {
                        const isDownloading = downloadingBackend === backend.backend;

                        // Determine status badge styling and icon
                        const getStatusBadge = () => {
                          switch (backend.status) {
                            case 'running':
                            case 'ready':
                              return (
                                <Badge variant="outline" className="bg-green-50 text-green-700 border-green-200">
                                  <CheckCircle2 className="w-3 h-3 mr-1" />
                                  Ready
                                </Badge>
                              );
                            case 'stopped':
                            case 'not_configured':
                              return (
                                <Badge variant="outline" className="bg-gray-50 text-gray-700 border-gray-200">
                                  <XCircle className="w-3 h-3 mr-1" />
                                  Not Configured
                                </Badge>
                              );
                            default:
                              return (
                                <Badge variant="outline" className="bg-yellow-50 text-yellow-700 border-yellow-200">
                                  <AlertCircle className="w-3 h-3 mr-1" />
                                  {backend.status}
                                </Badge>
                              );
                          }
                        };

                        return (
                          <TableRow key={backend.backend}>
                            <TableCell className="font-medium">{backend.backend}</TableCell>
                            <TableCell>
                              {getStatusBadge()}
                            </TableCell>
                            <TableCell className="flex gap-2">
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => getBackendStatus(backend.backend)}
                                disabled={isDownloading}
                              >
                                Check Status
                              </Button>
                              {backend.status !== 'running' && backend.status !== 'ready' && (
                                <Button
                                  variant="default"
                                  size="sm"
                                  onClick={() => downloadBackend(backend.backend)}
                                  disabled={isDownloading}
                                >
                                  {isDownloading ? (
                                    <>
                                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                                      Downloading...
                                    </>
                                  ) : (
                                    'Download'
                                  )}
                                </Button>
                              )}
                            </TableCell>
                          </TableRow>
                        );
                      })
                    ) : (
                      <TableRow>
                        <TableCell colSpan={3} className="text-center py-4">
                          No backends found
                        </TableCell>
                      </TableRow>
                    )}
                  </TableBody>
                </Table>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      <Dialog
        open={isModelDialogOpen}
        onOpenChange={(open) => {
          if (!open) {
            resetModelDialog();
          } else {
            setIsModelDialogOpen(true);
          }
        }}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{editingModelId ? 'Edit Model' : 'Add Model'}</DialogTitle>
            <DialogDescription>
              {editingModelId
                ? 'Update this model catalog entry.'
                : 'Create a model catalog entry for download and workflow selection.'}
            </DialogDescription>
          </DialogHeader>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void saveModel();
            }}
            className="space-y-4"
          >
            <div className="grid gap-2">
              <Label>Display Name</Label>
              <Input
                value={modelDraft.display_name}
                onChange={(e) => setModelField('display_name', e.target.value)}
                placeholder="Qwen2.5 0.5B Instruct (Q4_K_M)"
              />
            </div>

            <div className="grid gap-2">
              <Label>Repository ID</Label>
              <Input
                value={modelDraft.repo_id}
                onChange={(e) => setModelField('repo_id', e.target.value)}
                placeholder="bartowski/Qwen2.5-0.5B-Instruct-GGUF"
              />
            </div>

            <div className="grid gap-2">
              <Label>Filename</Label>
              <Input
                value={modelDraft.filename}
                onChange={(e) => setModelField('filename', e.target.value)}
                placeholder="Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"
              />
            </div>

            <div className="grid gap-2">
              <Label>Compatible Backends</Label>
              <div className="grid grid-cols-1 gap-2 rounded-md border p-3 sm:grid-cols-2">
                {availableBackendIds.map((backendId) => (
                  <label key={backendId} className="flex items-center gap-2 text-sm">
                    <Checkbox
                      checked={modelDraft.backend_ids.includes(backendId)}
                      onCheckedChange={(checked) => toggleBackendId(backendId, Boolean(checked))}
                    />
                    <span>{backendId}</span>
                  </label>
                ))}
              </div>
            </div>

            <DialogFooter>
              <Button type="button" variant="outline" onClick={resetModelDialog}>
                Cancel
              </Button>
              <Button type="submit" disabled={isSavingModel}>
                {isSavingModel && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                {editingModelId ? 'Save Changes' : 'Add Model'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      </div>
    </div>
  );
}
