import {
  startTransition,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from 'react';
import { useLocation, useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  ArrowRight,
  Boxes,
  Cloud,
  Cpu,
  Database,
  Download,
  Info,
  Layers2,
  Loader2,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Server,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { toast } from 'sonner';

import api, { getErrorMessage } from '@/lib/api';
import type { paths } from '@/lib/api';
import { cn } from '@/lib/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
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
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from '@/components/ui/resizable';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Separator } from '@/components/ui/separator';
import { Switch } from '@/components/ui/switch';

type SettingsSection = 'all' | 'runtime' | 'models' | 'chat_providers' | 'diffusion' | 'backends' | 'system';
type ModelStatus = 'downloaded' | 'pending' | 'not_downloaded';
type StatusFilter = 'all' | ModelStatus;

type SettingItem =
  paths['/v1/settings']['get']['responses'][200]['content']['application/json'][number];
type SettingsSystemResponse =
  paths['/v1/settings/system']['get']['responses'][200]['content']['application/json'];
type SettingsSystemBackend = SettingsSystemResponse['backends'][number];
type BackendListItem =
  paths['/v1/backends']['get']['responses'][200]['content']['application/json']['backends'][number];
type ModelCatalogItem =
  paths['/v1/models']['get']['responses'][200]['content']['application/json'][number];

type CloudProviderDraft = {
  id: string;
  name: string;
  api_base: string;
  api_key: string;
  api_key_env: string;
  models: CloudProviderModelDraft[];
};

type CloudProviderModelDraft = {
  id: string;
  display_name: string;
  remote_model: string;
};

type ModelDraft = {
  display_name: string;
  repo_id: string;
  filename: string;
  backend_ids: string[];
};

type BackendDownloadDraft = {
  backend_id: string;
  target_dir: string;
};

type BackendReloadDraft = {
  backend_id: string;
  lib_path: string;
  model_path: string;
  num_workers: string;
};

const CHAT_MODEL_PROVIDERS_KEY = 'chat_model_providers';
const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

const EMPTY_MODEL_DRAFT: ModelDraft = {
  display_name: '',
  repo_id: '',
  filename: '',
  backend_ids: [],
};

const EMPTY_BACKEND_DOWNLOAD: BackendDownloadDraft = {
  backend_id: '',
  target_dir: '',
};

const EMPTY_BACKEND_RELOAD: BackendReloadDraft = {
  backend_id: '',
  lib_path: '',
  model_path: '',
  num_workers: '1',
};

const SECTION_META: Array<{
  id: SettingsSection;
  title: string;
  description: string;
  icon: typeof Info;
}> = [
  { id: 'all', title: '全部结果', description: '总览与搜索结果', icon: Search },
  { id: 'runtime', title: '运行时设置', description: '缓存、worker 与自动卸载', icon: Cpu },
  { id: 'models', title: '模型目录', description: '模型条目与下载管理', icon: Boxes },
  { id: 'chat_providers', title: '聊天提供商', description: '云模型映射与凭据', icon: Cloud },
  { id: 'diffusion', title: '扩散', description: 'Diffusion 路径与性能选项', icon: Sparkles },
  { id: 'backends', title: '后端', description: '状态、下载与重载', icon: Server },
  { id: 'system', title: '系统信息', description: '只读运行事实', icon: Info },
];

function getSectionFromSearchParams(searchParams: URLSearchParams): SettingsSection {
  const value = searchParams.get('section');
  if (!value) return 'all';
  return SECTION_META.some((item) => item.id === value) ? (value as SettingsSection) : 'all';
}

function updateSearchParams(
  searchParams: URLSearchParams,
  update: Partial<{ q: string; section: SettingsSection }>,
) {
  const next = new URLSearchParams(searchParams);
  if (update.q !== undefined) {
    if (update.q.trim()) next.set('q', update.q);
    else next.delete('q');
  }
  if (update.section !== undefined) {
    if (update.section === 'all') next.delete('section');
    else next.set('section', update.section);
  }
  return next;
}

function getTargetFromHash(hash: string) {
  if (!hash.startsWith('#')) return '';
  try {
    return decodeURIComponent(hash.slice(1));
  } catch {
    return hash.slice(1);
  }
}

function toTextValue(value: unknown) {
  if (typeof value === 'string') return value;
  if (typeof value === 'number') return String(value);
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  return '';
}

function toBooleanValue(value: unknown) {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'string') return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase());
  return false;
}

function toDisplayValue(value: unknown) {
  if (value === null || value === undefined || value === '') return '未设置';
  if (typeof value === 'string') return value;
  if (typeof value === 'number') return String(value);
  if (typeof value === 'boolean') return value ? '已启用' : '已禁用';
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function toCloudProviderDrafts(value: unknown): CloudProviderDraft[] {
  if (!Array.isArray(value)) return [];
  return value.map((provider) => {
    const safeProvider = typeof provider === 'object' && provider !== null ? provider : {};
    const models = Array.isArray((safeProvider as { models?: unknown }).models)
      ? ((safeProvider as { models: unknown[] }).models ?? []).map((model) => {
          const safeModel = typeof model === 'object' && model !== null ? model : {};
          return {
            id: toTextValue((safeModel as { id?: unknown }).id),
            display_name: toTextValue((safeModel as { display_name?: unknown }).display_name),
            remote_model: toTextValue((safeModel as { remote_model?: unknown }).remote_model),
          };
        })
      : [];
    return {
      id: toTextValue((safeProvider as { id?: unknown }).id),
      name: toTextValue((safeProvider as { name?: unknown }).name),
      api_base: toTextValue((safeProvider as { api_base?: unknown }).api_base),
      api_key: toTextValue((safeProvider as { api_key?: unknown }).api_key),
      api_key_env: toTextValue((safeProvider as { api_key_env?: unknown }).api_key_env),
      models,
    };
  });
}

function maskSecret(secret: string) {
  const trimmed = secret.trim();
  if (!trimmed) return '未设置';
  if (trimmed.length <= 6) return '••••••';
  return `${trimmed.slice(0, 3)}••••${trimmed.slice(-2)}`;
}

function matchesSearch(query: string, values: unknown[]) {
  if (!query) return true;
  const haystack = values
    .map((value) => {
      if (value === null || value === undefined) return '';
      if (typeof value === 'string') return value;
      if (typeof value === 'number' || typeof value === 'boolean') return String(value);
      try {
        return JSON.stringify(value);
      } catch {
        return '';
      }
    })
    .join(' ')
    .toLowerCase();
  return haystack.includes(query);
}

function modelStatusOf(model: ModelCatalogItem): ModelStatus {
  if (model.local_path) return 'downloaded';
  if (typeof model.pending_task_id === 'string' && model.pending_task_id.trim()) return 'pending';
  return 'not_downloaded';
}

function formatDate(value?: string | null) {
  if (!value) return '从未';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return parsed.toLocaleString();
}

function statusBadge(status: ModelStatus) {
  if (status === 'downloaded') return <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700">已下载</Badge>;
  if (status === 'pending') return <Badge variant="secondary">下载中</Badge>;
  return <Badge variant="outline">未下载</Badge>;
}

function settingSectionOf(setting: SettingItem): SettingsSection {
  if (setting.category === 'runtime') return 'runtime';
  if (setting.category === 'chat_providers') return 'chat_providers';
  if (setting.category === 'diffusion') return 'diffusion';
  return 'all';
}

function settingSectionLabel(section: SettingsSection) {
  return SECTION_META.find((item) => item.id === section)?.title ?? '全部结果';
}

export default function Settings() {
  const { t } = useTranslation();
  const location = useLocation();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const [searchValue, setSearchValue] = useState(searchParams.get('q') ?? '');
  const section = getSectionFromSearchParams(searchParams);
  const deferredQuery = useDeferredValue(searchValue.trim().toLowerCase());
  const highlightedTarget = getTargetFromHash(location.hash);

  const [savingSettingKey, setSavingSettingKey] = useState<string | null>(null);
  const [modelDialogOpen, setModelDialogOpen] = useState(false);
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [modelDraft, setModelDraft] = useState<ModelDraft>(EMPTY_MODEL_DRAFT);
  const [deletingModelId, setDeletingModelId] = useState<string | null>(null);
  const [busyModelId, setBusyModelId] = useState<string | null>(null);
  const [catalogBackendFilter, setCatalogBackendFilter] = useState('all');
  const [catalogStatusFilter, setCatalogStatusFilter] = useState<StatusFilter>('all');
  const [providersDraft, setProvidersDraft] = useState<CloudProviderDraft[]>([]);
  const [savingProviders, setSavingProviders] = useState(false);
  const [downloadDraft, setDownloadDraft] = useState<BackendDownloadDraft>(EMPTY_BACKEND_DOWNLOAD);
  const [downloadDialogOpen, setDownloadDialogOpen] = useState(false);
  const [reloadDraft, setReloadDraft] = useState<BackendReloadDraft>(EMPTY_BACKEND_RELOAD);
  const [reloadDialogOpen, setReloadDialogOpen] = useState(false);
  const [activeBackendId, setActiveBackendId] = useState<string | null>(null);

  const { data: settingsData, isLoading: settingsLoading, refetch: refetchSettings } = api.useQuery('get', '/v1/settings');
  const { data: systemData, isLoading: systemLoading, refetch: refetchSystem } = api.useQuery('get', '/v1/settings/system');
  const { data: modelsData, isLoading: modelsLoading, refetch: refetchModels } = api.useQuery('get', '/v1/models');
  const { data: backendsData, isLoading: backendsLoading, refetch: refetchBackends } = api.useQuery('get', '/v1/backends');

  const updateSettingMutation = api.useMutation('put', '/v1/settings/{key}');
  const createModelMutation = api.useMutation('post', '/v1/models');
  const updateModelMutation = api.useMutation('put', '/v1/models/{id}');
  const deleteModelMutation = api.useMutation('delete', '/v1/models/{id}');
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');
  const downloadBackendMutation = api.useMutation('post', '/v1/backends/download');
  const reloadBackendMutation = api.useMutation('post', '/v1/backends/reload');
  const backendStatusMutation = api.useMutation('get', '/v1/backends/status');

  const settings = Array.isArray(settingsData) ? settingsData : [];
  const models = Array.isArray(modelsData) ? modelsData : [];
  const backendList =
    typeof backendsData === 'object' &&
    backendsData !== null &&
    Array.isArray((backendsData as { backends?: unknown }).backends)
      ? ((backendsData as { backends: BackendListItem[] }).backends ?? [])
      : [];
  const system = (systemData ?? null) as SettingsSystemResponse | null;

  const systemBackendMap = useMemo(
    () => new Map((system?.backends ?? []).map((backend) => [backend.backend, backend])),
    [system],
  );

  const availableBackendIds = useMemo(() => {
    const ids = new Set<string>();
    for (const backend of backendList) ids.add(backend.backend);
    if (ids.size === 0) {
      ids.add('ggml.llama');
      ids.add('ggml.whisper');
      ids.add('ggml.diffusion');
    }
    return Array.from(ids).sort();
  }, [backendList]);

  const settingsBySection = useMemo(() => {
    const groups: Record<'runtime' | 'chat_providers' | 'diffusion', SettingItem[]> = {
      runtime: [],
      chat_providers: [],
      diffusion: [],
    };
    for (const setting of settings) {
      if (setting.category === 'runtime') groups.runtime.push(setting);
      if (setting.category === 'chat_providers') groups.chat_providers.push(setting);
      if (setting.category === 'diffusion') groups.diffusion.push(setting);
    }
    return groups;
  }, [settings]);

  const chatProvidersSetting = settings.find((setting) => setting.key === CHAT_MODEL_PROVIDERS_KEY);

  useEffect(() => {
    setSearchValue(searchParams.get('q') ?? '');
  }, [searchParams]);

  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key !== '/' || event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement | null;
      const tagName = target?.tagName?.toLowerCase();
      const isEditable =
        tagName === 'input' ||
        tagName === 'textarea' ||
        target?.isContentEditable === true;
      if (isEditable) return;
      event.preventDefault();
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  useEffect(() => {
    setProvidersDraft(toCloudProviderDrafts(chatProvidersSetting?.effective_value));
  }, [chatProvidersSetting?.effective_value]);

  useEffect(() => {
    if (!highlightedTarget) return;
    const raf = window.requestAnimationFrame(() => {
      const element = document.getElementById(highlightedTarget);
      if (!element) return;
      element.scrollIntoView({ block: 'center', behavior: 'smooth' });
    });
    return () => window.cancelAnimationFrame(raf);
  }, [highlightedTarget, section]);

  const filteredSettings = useMemo(
    () =>
      settings.filter((setting) =>
        matchesSearch(deferredQuery, [
          setting.label,
          setting.key,
          setting.description,
          ...setting.search_terms,
          setting.effective_value,
        ]),
      ),
    [deferredQuery, settings],
  );

  const filteredModels = useMemo(
    () =>
      models.filter((model) => {
        const status = modelStatusOf(model);
        if (catalogBackendFilter !== 'all' && !model.backend_ids.includes(catalogBackendFilter)) return false;
        if (catalogStatusFilter !== 'all' && status !== catalogStatusFilter) return false;
        return matchesSearch(deferredQuery, [
          model.display_name,
          model.id,
          model.repo_id,
          model.filename,
          model.backend_ids.join(' '),
          model.local_path ?? '',
          status,
        ]);
      }),
    [catalogBackendFilter, catalogStatusFilter, deferredQuery, models],
  );

  const backendRows = useMemo(
    () =>
      backendList.filter((backend) => {
        const systemBackend = systemBackendMap.get(backend.backend);
        return matchesSearch(deferredQuery, [
          backend.backend,
          backend.status,
          systemBackend?.endpoint ?? '',
          systemBackend?.runtime_status ?? '',
          systemBackend?.worker_setting_key ?? '',
        ]);
      }),
    [backendList, deferredQuery, systemBackendMap],
  );

  const installedModelCount = models.filter((model) => Boolean(model.local_path)).length;
  const downloadingModelCount = models.filter((model) => modelStatusOf(model) === 'pending').length;
  const updateQuery = (next: Partial<{ q: string; section: SettingsSection }>) => {
    startTransition(() => {
      setSearchParams(updateSearchParams(searchParams, next), { replace: true });
    });
  };

  const setHighlightTarget = (targetId?: string) => {
    const nextSearch = searchParams.toString();
    navigate(
      {
        pathname: location.pathname,
        search: nextSearch ? `?${nextSearch}` : '',
        hash: targetId ? `#${encodeURIComponent(targetId)}` : '',
      },
      { replace: true },
    );
  };

  const openSectionTarget = (nextSection: SettingsSection, targetId: string) => {
    const nextSearchParams = updateSearchParams(searchParams, { section: nextSection });
    if (highlightedTarget === targetId) {
      const element = document.getElementById(targetId);
      element?.scrollIntoView({ block: 'center', behavior: 'smooth' });
    }
    navigate(
      {
        pathname: location.pathname,
        search: nextSearchParams.toString() ? `?${nextSearchParams.toString()}` : '',
        hash: `#${encodeURIComponent(targetId)}`,
      },
      { replace: true },
    );
  };

  const openSection = (nextSection: SettingsSection) => {
    setHighlightTarget(undefined);
    updateQuery({ section: nextSection });
  };

  const saveSetting = async (setting: SettingItem, value: unknown) => {
    setSavingSettingKey(setting.key);
    try {
      await updateSettingMutation.mutateAsync({
        params: { path: { key: setting.key } },
        body: { value },
      });
      toast.success(`${setting.label} 已保存`);
      await refetchSettings();
      await refetchSystem();
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setSavingSettingKey(null);
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

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;
    while (Date.now() < deadline) {
      const task = await getTaskMutation.mutateAsync({ params: { path: { id: taskId } } });
      if (task.status === 'succeeded') return;
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
    let model = models.find((item) => item.id === modelId);
    if (!model) model = await refreshCatalogAndFindModel(modelId);
    if (!model) throw new Error('Selected model does not exist in catalog');
    if (model.local_path) return model.local_path;
    const pendingTaskId =
      typeof model.pending_task_id === 'string' && model.pending_task_id.trim()
        ? model.pending_task_id
        : null;
    let taskId = pendingTaskId;
    if (!taskId) {
      const downloadResponse = await downloadModelMutation.mutateAsync({
        body: { backend_id: backendId, model_id: modelId },
      });
      taskId = extractTaskId(downloadResponse);
    }
    if (!taskId) throw new Error('Failed to start model download task');
    await waitForTaskToFinish(taskId);
    const refreshedModel = await refreshCatalogAndFindModel(modelId);
    if (!refreshedModel?.local_path) throw new Error('Model download completed, but local_path is empty');
    return refreshedModel.local_path;
  };

  const openCreateModelDialog = () => {
    setEditingModelId(null);
    setModelDraft({
      ...EMPTY_MODEL_DRAFT,
      backend_ids: availableBackendIds.length > 0 ? [availableBackendIds[0]] : [],
    });
    setModelDialogOpen(true);
  };

  const openEditModelDialog = (model: ModelCatalogItem) => {
    setEditingModelId(model.id);
    setModelDraft({
      display_name: model.display_name,
      repo_id: model.repo_id,
      filename: model.filename,
      backend_ids: model.backend_ids,
    });
    setModelDialogOpen(true);
  };

  const saveModel = async () => {
    const display_name = modelDraft.display_name.trim();
    const repo_id = modelDraft.repo_id.trim();
    const filename = modelDraft.filename.trim();
    const backend_ids = modelDraft.backend_ids;
    if (!display_name || !repo_id || !filename) {
      toast.error('Display name、repo id 和 filename 不能为空');
      return;
    }
    if (backend_ids.length === 0) {
      toast.error('至少选择一个 backend');
      return;
    }
    try {
      if (editingModelId) {
        await updateModelMutation.mutateAsync({
          params: { path: { id: editingModelId } },
          body: { display_name, repo_id, filename, backend_ids },
        });
        toast.success('模型条目已更新');
      } else {
        await createModelMutation.mutateAsync({
          body: { display_name, repo_id, filename, backend_ids },
        });
        toast.success('模型条目已创建');
      }
      await refetchModels();
      setModelDialogOpen(false);
      setEditingModelId(null);
      setModelDraft(EMPTY_MODEL_DRAFT);
    } catch (error) {
      toast.error(getErrorMessage(error));
    }
  };

  const deleteModel = async (id: string) => {
    setDeletingModelId(id);
    try {
      await deleteModelMutation.mutateAsync({
        params: { path: { id } },
      });
      toast.success('模型条目已删除');
      await refetchModels();
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setDeletingModelId(null);
    }
  };

  const downloadModel = async (model: ModelCatalogItem) => {
    const backendId = model.backend_ids[0] ?? '';
    if (!backendId) {
      toast.error('当前模型没有可用 backend');
      return;
    }
    setBusyModelId(model.id);
    try {
      await ensureDownloadedModelPath(model.id, backendId);
      toast.success(`${model.display_name} 下载完成`);
      await refetchModels();
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setBusyModelId(null);
    }
  };

  const addProvider = () => {
    setProvidersDraft((prev) => [
      ...prev,
      {
        id: '',
        name: '',
        api_base: '',
        api_key: '',
        api_key_env: '',
        models: [{ id: '', display_name: '', remote_model: '' }],
      },
    ]);
  };

  const saveProviders = async () => {
    if (!chatProvidersSetting) return;
    setSavingProviders(true);
    try {
      await saveSetting(
        chatProvidersSetting,
        providersDraft.map((provider) => ({
          id: provider.id,
          name: provider.name,
          api_base: provider.api_base,
          api_key: provider.api_key || undefined,
          api_key_env: provider.api_key_env || undefined,
          models: provider.models.map((model) => ({
            id: model.id,
            display_name: model.display_name,
            remote_model: model.remote_model || undefined,
          })),
        })),
      );
    } finally {
      setSavingProviders(false);
    }
  };

  const refreshConsole = async () => {
    await Promise.all([refetchSettings(), refetchModels(), refetchBackends(), refetchSystem()]);
  };

  const openBackendDownloadDialog = (backendId: string) => {
    setDownloadDraft({ backend_id: backendId, target_dir: '' });
    setDownloadDialogOpen(true);
  };

  const submitBackendDownload = async () => {
    if (!downloadDraft.backend_id || !downloadDraft.target_dir.trim()) {
      toast.error('backend 和 target_dir 不能为空');
      return;
    }
    setActiveBackendId(downloadDraft.backend_id);
    try {
      await downloadBackendMutation.mutateAsync({
        body: {
          backend_id: downloadDraft.backend_id,
          target_dir: downloadDraft.target_dir.trim(),
        },
      });
      toast.success(`已提交 ${downloadDraft.backend_id} 下载任务`);
      setDownloadDialogOpen(false);
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setActiveBackendId(null);
    }
  };

  const openBackendReloadDialog = (backendId: string) => {
    const systemBackend = systemBackendMap.get(backendId);
    setReloadDraft({
      backend_id: backendId,
      lib_path: '',
      model_path: '',
      num_workers: String(systemBackend?.effective_workers ?? 1),
    });
    setReloadDialogOpen(true);
  };

  const submitBackendReload = async () => {
    const workers = Number.parseInt(reloadDraft.num_workers, 10);
    if (!reloadDraft.backend_id || !reloadDraft.lib_path.trim() || !reloadDraft.model_path.trim()) {
      toast.error('backend、lib_path 和 model_path 不能为空');
      return;
    }
    if (!Number.isFinite(workers) || workers < 1) {
      toast.error('num_workers 必须大于等于 1');
      return;
    }
    setActiveBackendId(reloadDraft.backend_id);
    try {
      await reloadBackendMutation.mutateAsync({
        body: {
          backend_id: reloadDraft.backend_id,
          lib_path: reloadDraft.lib_path.trim(),
          model_path: reloadDraft.model_path.trim(),
          num_workers: workers,
        },
      });
      toast.success(`${reloadDraft.backend_id} 已重新加载`);
      await Promise.all([refetchBackends(), refetchSystem()]);
      setReloadDialogOpen(false);
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setActiveBackendId(null);
    }
  };

  const checkBackendStatus = async (backendId: string) => {
    setActiveBackendId(backendId);
    try {
      const response = await backendStatusMutation.mutateAsync({
        params: { query: { backend_id: backendId } },
      });
      toast.success(`${backendId}: ${response.status}`);
      await Promise.all([refetchBackends(), refetchSystem()]);
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setActiveBackendId(null);
    }
  };

  return (
    <div className="h-full overflow-hidden">
      <div className="flex h-full flex-col">
        <div className="sticky top-0 z-20 border-b border-border/60 bg-background/92 backdrop-blur-xl">
          <div className="mx-auto flex max-w-[1600px] flex-col gap-4 px-4 py-4">
            <div className="rounded-[28px] border border-border/70 bg-[radial-gradient(circle_at_top_left,_color-mix(in_oklab,_var(--primary)_14%,_transparent),_transparent_34%),linear-gradient(180deg,color-mix(in_oklab,var(--card)_82%,transparent),var(--card))] px-5 py-5 shadow-[0_18px_60px_-40px_color-mix(in_oklab,var(--foreground)_36%,transparent)]">
            <div className="mt-4 grid gap-3 border-t border-border/60 pt-4 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center">
              <div className="relative">
                <Search className="pointer-events-none absolute left-4 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  ref={searchInputRef}
                  value={searchValue}
                  onChange={(event) => {
                    const next = event.target.value;
                    setSearchValue(next);
                    updateQuery({ q: next });
                  }}
                  placeholder={t('settings.searchPlaceholder', '搜索设置、模型或后端…')}
                  className="h-12 rounded-2xl border-border/70 bg-background/85 pl-11 pr-24 shadow-sm"
                />
                <div className="pointer-events-none absolute right-3 top-1/2 flex -translate-y-1/2 items-center gap-1">
                  <Badge variant="outline" className="rounded-md px-2 py-0.5 font-mono text-[11px] text-muted-foreground">
                    /
                  </Badge>
                  <Badge variant="outline" className="rounded-md px-2 py-0.5 font-mono text-[11px] text-muted-foreground">
                    Enter
                  </Badge>
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <Badge variant="outline" className="rounded-full px-3 py-1">Settings {settings.length}</Badge>
                <Badge variant="outline" className="rounded-full px-3 py-1">Models {models.length}</Badge>
                <Badge variant="outline" className="rounded-full px-3 py-1">Backends {backendList.length}</Badge>
              </div>
            </div>
          </div>
        </div>
      </div>

        <div className="min-h-0 flex-1">
          <ResizablePanelGroup orientation="horizontal">
            <ResizablePanel defaultSize="22%" minSize="18%" maxSize="28%">
              <div className="h-full border-r border-border/60 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--muted)_72%,transparent),transparent_28%)]">
                <div className="flex h-full flex-col">
                  <div className="border-b border-border/60 px-4 py-4">
                    <p className="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">
                      Settings Index
                    </p>
                    <p className="mt-2 text-xs text-muted-foreground">
                      {t('settings.indexHint', 'Switch sections and keep the current workspace anchored.')}
                    </p>
                  </div>
                  <div className="flex-1 overflow-y-auto px-3 py-3">
                    <div className="space-y-2">
                      {SECTION_META.map((item) => {
                        const Icon = item.icon;
                        const count = item.id === 'all'
                          ? deferredQuery
                            ? filteredSettings.length + filteredModels.length + backendRows.length
                            : settings.length + models.length + backendList.length
                          : item.id === 'runtime'
                            ? filteredSettings.filter((setting) => setting.category === 'runtime').length
                            : item.id === 'chat_providers'
                              ? providersDraft.length
                              : item.id === 'diffusion'
                                ? filteredSettings.filter((setting) => setting.category === 'diffusion').length
                                : item.id === 'models'
                                  ? filteredModels.length
                                  : item.id === 'backends'
                                    ? backendRows.length
                                    : system?.backends.length ?? 0;
                        return (
                          <button
                            key={item.id}
                            type="button"
                            aria-current={section === item.id ? 'page' : undefined}
                            onClick={() => {
                              setHighlightTarget(undefined);
                              updateQuery({ section: item.id });
                            }}
                            className={cn(
                              'w-full rounded-2xl border px-3 py-3 text-left transition-all duration-200',
                              section === item.id
                                ? 'border-primary/30 bg-primary/10 shadow-[0_12px_30px_-24px_color-mix(in_oklab,var(--primary)_80%,transparent)]'
                                : 'border-transparent hover:border-border/80 hover:bg-background',
                            )}
                          >
                            <div className="flex items-start justify-between gap-3">
                              <div className="space-y-1">
                                <div className="flex items-center gap-2">
                                  <div className={cn(
                                    'rounded-xl border border-transparent p-1.5',
                                    section === item.id ? 'bg-background/80 text-primary' : 'bg-background/60 text-muted-foreground',
                                  )}>
                                    <Icon className="h-4 w-4" />
                                  </div>
                                  <span className="font-medium">{item.title}</span>
                                </div>
                                <p className="text-xs text-muted-foreground">{item.description}</p>
                              </div>
                              <Badge variant={section === item.id ? 'default' : 'secondary'} className="rounded-full px-2.5">
                                {count}
                              </Badge>
                            </div>
                          </button>
                        );
                      })}
                    </div>
                  </div>
                </div>
              </div>
            </ResizablePanel>
            <ResizableHandle withHandle />
            <ResizablePanel defaultSize="78%">
              <div className="h-full overflow-y-auto">
                <div className="mx-auto max-w-[1200px] space-y-6 px-4 py-6">
                  {(settingsLoading || modelsLoading || backendsLoading || systemLoading) && (
                    <Card>
                      <CardContent className="flex items-center gap-3 py-8 text-sm text-muted-foreground">
                        <Loader2 className="h-4 w-4 animate-spin" />
                        正在加载 Settings 控制台数据…
                      </CardContent>
                    </Card>
                  )}

                  {!settingsLoading && !modelsLoading && !backendsLoading && !systemLoading && (
                    <>
                      {deferredQuery && section === 'all' && (
                        <div className="space-y-6">
                          <SearchResultsGroup title="设置项" count={filteredSettings.length} emptyText="没有命中的设置项">
                            {filteredSettings.map((setting) => (
                              <SearchResultCard
                                key={setting.key}
                                title={setting.label}
                                description={setting.description}
                                meta={`${setting.key} · ${settingSectionLabel(settingSectionOf(setting))}`}
                                value={toDisplayValue(setting.effective_value)}
                                onOpen={() => openSectionTarget(settingSectionOf(setting), `setting-${setting.key}`)}
                              />
                            ))}
                          </SearchResultsGroup>
                          <SearchResultsGroup title="模型目录" count={filteredModels.length} emptyText="没有命中的模型条目">
                            {filteredModels.map((model) => (
                              <SearchResultCard
                                key={model.id}
                                title={model.display_name}
                                description={`${model.repo_id} / ${model.filename}`}
                                meta={`${model.id} · ${model.backend_ids.join(', ')}`}
                                value={statusBadge(modelStatusOf(model))}
                                onOpen={() => openSectionTarget('models', `model-${model.id}`)}
                              />
                            ))}
                          </SearchResultsGroup>
                          <SearchResultsGroup title="后端" count={backendRows.length} emptyText="没有命中的后端条目">
                            {backendRows.map((backend) => (
                              <SearchResultCard
                                key={backend.backend}
                                title={backend.backend}
                                description={`运行状态: ${backend.status}`}
                                meta={systemBackendMap.get(backend.backend)?.endpoint ?? '未配置 endpoint'}
                                value={backend.status}
                                onOpen={() => openSectionTarget('backends', `backend-${backend.backend}`)}
                              />
                            ))}
                          </SearchResultsGroup>
                        </div>
                      )}

                      {!deferredQuery && section === 'all' && (
                        <div className="space-y-6">
                          <div className="grid gap-4 lg:grid-cols-4">
                            <OverviewCard title="运行时设置" value={settingsBySection.runtime.length} description="registry 驱动的可编辑设置项" actionLabel="打开" onClick={() => openSection('runtime')} icon={Cpu} />
                            <OverviewCard title="模型目录" value={models.length} description={`${installedModelCount} 已下载，${downloadingModelCount} 下载中`} actionLabel="管理模型" onClick={() => openSection('models')} icon={Boxes} />
                            <OverviewCard title="聊天提供商" value={providersDraft.length} description="结构化编辑云提供商与模型映射" actionLabel="编辑" onClick={() => openSection('chat_providers')} icon={Cloud} />
                            <OverviewCard title="后端" value={backendList.length} description={`${backendRows.filter((backend) => backend.status === 'ready').length} 个 ready`} actionLabel="查看状态" onClick={() => openSection('backends')} icon={Server} />
                          </div>
                          <Card>
                            <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                              <div>
                                <CardTitle>控制台概览</CardTitle>
                                <CardDescription>这里聚合了可编辑设置、模型目录、后端状态和系统只读信息。</CardDescription>
                              </div>
                              <Button variant="outline" onClick={() => void refreshConsole()}>
                                <RefreshCw className="mr-2 h-4 w-4" />
                                刷新所有数据
                              </Button>
                            </CardHeader>
                            <CardContent className="grid gap-4 md:grid-cols-2">
                              <CompactSummaryCard title="运行事实" lines={[`Transport: ${system?.transport_mode ?? '加载中'}`, `Bind: ${system?.bind_address ?? '加载中'}`, `Swagger: ${system?.swagger_enabled ? '开启' : '关闭'}`, `Admin Token: ${system?.admin_token_enabled ? '已配置' : '未配置'}`]} />
                              <CompactSummaryCard title="导航建议" lines={['在左侧分类中切换工作区', '顶部搜索会同时筛选设置项、模型和后端', '复杂设置采用结构化表单，不再暴露通用 key/value 表格', 'Hub 页面只保留资源发现入口']} />
                            </CardContent>
                          </Card>
                        </div>
                      )}

                      {section === 'runtime' && <SettingsCategoryPanel title="运行时设置" description="控制缓存目录、worker 数、上下文长度和自动卸载。" settings={filteredSettings.filter((setting) => setting.category === 'runtime')} highlightedTarget={highlightedTarget} savingSettingKey={savingSettingKey} onSave={saveSetting} />}
                      {section === 'diffusion' && <SettingsCategoryPanel title="Diffusion 设置" description="统一管理 Diffusion 路径和性能开关。" settings={filteredSettings.filter((setting) => setting.category === 'diffusion')} highlightedTarget={highlightedTarget} savingSettingKey={savingSettingKey} onSave={saveSetting} />}
                      {section === 'chat_providers' && <ChatProvidersPanel providers={providersDraft} query={deferredQuery} saving={savingProviders} onAddProvider={addProvider} onChange={setProvidersDraft} onSave={() => void saveProviders()} />}
                      {section === 'models' && <ModelCatalogPanel models={filteredModels} totalModels={models.length} installedModelCount={installedModelCount} downloadingModelCount={downloadingModelCount} highlightedTarget={highlightedTarget} backendFilter={catalogBackendFilter} onBackendFilterChange={setCatalogBackendFilter} statusFilter={catalogStatusFilter} onStatusFilterChange={setCatalogStatusFilter} backendOptions={availableBackendIds} busyModelId={busyModelId} deletingModelId={deletingModelId} onRefresh={() => void refetchModels()} onCreateModel={openCreateModelDialog} onEditModel={openEditModelDialog} onDeleteModel={(id) => void deleteModel(id)} onDownloadModel={(model) => void downloadModel(model)} />}
                      {section === 'backends' && <BackendsPanel backends={backendRows} systemBackendMap={systemBackendMap} highlightedTarget={highlightedTarget} activeBackendId={activeBackendId} onRefresh={() => void Promise.all([refetchBackends(), refetchSystem()])} onCheckStatus={(backendId) => void checkBackendStatus(backendId)} onDownloadLib={openBackendDownloadDialog} onReloadLib={openBackendReloadDialog} />}
                      {section === 'system' && <SystemInfoPanel system={system} />}
                    </>
                  )}
                </div>
              </div>
            </ResizablePanel>
          </ResizablePanelGroup>
        </div>
      </div>

      <Dialog open={modelDialogOpen} onOpenChange={setModelDialogOpen}>
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{editingModelId ? '编辑模型条目' : '新增模型条目'}</DialogTitle>
            <DialogDescription>保存到模型目录后，下载和工作流选择都会使用这条记录。</DialogDescription>
          </DialogHeader>
          <form className="space-y-4" onSubmit={(event) => { event.preventDefault(); void saveModel(); }}>
            <div className="grid gap-2">
              <Label>Display Name</Label>
              <Input value={modelDraft.display_name} onChange={(event) => setModelDraft((prev) => ({ ...prev, display_name: event.target.value }))} placeholder="Qwen2.5 0.5B Instruct (Q4_K_M)" />
            </div>
            <div className="grid gap-2">
              <Label>Repository ID</Label>
              <Input value={modelDraft.repo_id} onChange={(event) => setModelDraft((prev) => ({ ...prev, repo_id: event.target.value }))} placeholder="bartowski/Qwen2.5-0.5B-Instruct-GGUF" />
            </div>
            <div className="grid gap-2">
              <Label>Filename</Label>
              <Input value={modelDraft.filename} onChange={(event) => setModelDraft((prev) => ({ ...prev, filename: event.target.value }))} placeholder="Qwen2.5-0.5B-Instruct-Q4_K_M.gguf" />
            </div>
            <div className="grid gap-2">
              <Label>Compatible Backends</Label>
              <div className="grid grid-cols-1 gap-2 rounded-xl border p-3 sm:grid-cols-2">
                {availableBackendIds.map((backendId) => (
                  <label key={backendId} className="flex items-center gap-2 text-sm">
                    <Checkbox
                      checked={modelDraft.backend_ids.includes(backendId)}
                      onCheckedChange={(checked) =>
                        setModelDraft((prev) => ({
                          ...prev,
                          backend_ids: checked
                            ? prev.backend_ids.includes(backendId)
                              ? prev.backend_ids
                              : [...prev.backend_ids, backendId]
                            : prev.backend_ids.filter((value) => value !== backendId),
                        }))
                      }
                    />
                    <span>{backendId}</span>
                  </label>
                ))}
              </div>
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setModelDialogOpen(false)}>取消</Button>
              <Button type="submit" disabled={createModelMutation.isPending || updateModelMutation.isPending}>
                {(createModelMutation.isPending || updateModelMutation.isPending) && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                {editingModelId ? '保存修改' : '创建模型'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={downloadDialogOpen} onOpenChange={setDownloadDialogOpen}>
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>下载 Backend 运行库</DialogTitle>
            <DialogDescription>提交 Windows 运行库下载任务。</DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="grid gap-2">
              <Label>Backend</Label>
              <Input value={downloadDraft.backend_id} disabled />
            </div>
            <div className="grid gap-2">
              <Label>Target Directory</Label>
              <Input value={downloadDraft.target_dir} onChange={(event) => setDownloadDraft((prev) => ({ ...prev, target_dir: event.target.value }))} placeholder="C:\\slab\\runtime\\llama" />
            </div>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setDownloadDialogOpen(false)}>取消</Button>
            <Button type="button" onClick={() => void submitBackendDownload()} disabled={downloadBackendMutation.isPending}>
              {downloadBackendMutation.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              提交下载
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={reloadDialogOpen} onOpenChange={setReloadDialogOpen}>
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>重载 Backend</DialogTitle>
            <DialogDescription>提供新的 lib 与 model 路径，重新加载指定 backend。</DialogDescription>
          </DialogHeader>
          <div className="space-y-4">
            <div className="grid gap-2">
              <Label>Backend</Label>
              <Input value={reloadDraft.backend_id} disabled />
            </div>
            <div className="grid gap-2">
              <Label>Library Path</Label>
              <Input value={reloadDraft.lib_path} onChange={(event) => setReloadDraft((prev) => ({ ...prev, lib_path: event.target.value }))} placeholder="C:\\slab\\runtime\\llama.dll" />
            </div>
            <div className="grid gap-2">
              <Label>Model Path</Label>
              <Input value={reloadDraft.model_path} onChange={(event) => setReloadDraft((prev) => ({ ...prev, model_path: event.target.value }))} placeholder="C:\\models\\Qwen.gguf" />
            </div>
            <div className="grid gap-2">
              <Label>Num Workers</Label>
              <Input type="number" min={1} value={reloadDraft.num_workers} onChange={(event) => setReloadDraft((prev) => ({ ...prev, num_workers: event.target.value }))} />
            </div>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setReloadDialogOpen(false)}>取消</Button>
            <Button type="button" onClick={() => void submitBackendReload()} disabled={reloadBackendMutation.isPending}>
              {reloadBackendMutation.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              重载 Backend
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function OverviewCard({
  title,
  value,
  description,
  actionLabel,
  onClick,
  icon: Icon,
}: {
  title: string;
  value: number;
  description: string;
  actionLabel: string;
  onClick: () => void;
  icon: typeof Info;
}) {
  return (
    <Card className="overflow-hidden border-border/70 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--card)_82%,transparent),var(--card))] shadow-[0_18px_50px_-42px_color-mix(in_oklab,var(--foreground)_40%,transparent)]">
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between gap-3">
          <div>
            <CardDescription>{title}</CardDescription>
            <CardTitle className="mt-2 text-3xl">{value}</CardTitle>
          </div>
          <div className="rounded-2xl border border-border/70 bg-background/70 p-2.5 shadow-sm">
            <Icon className="h-4 w-4" />
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        <p className="text-sm text-muted-foreground">{description}</p>
        <Button variant="outline" size="sm" className="rounded-full" onClick={onClick}>
          {actionLabel}
        </Button>
      </CardContent>
    </Card>
  );
}

function CompactSummaryCard({ title, lines }: { title: string; lines: string[] }) {
  return (
    <Card className="border-border/70">
      <CardHeader className="border-b border-border/60">
        <CardTitle className="text-base">{title}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 pt-4 text-sm text-muted-foreground">
        {lines.map((line) => (
          <p key={line} className="rounded-xl bg-muted/40 px-3 py-2">{line}</p>
        ))}
      </CardContent>
    </Card>
  );
}

function SearchResultsGroup({
  title,
  count,
  emptyText,
  children,
}: {
  title: string;
  count: number;
  emptyText: string;
  children: ReactNode;
}) {
  return (
    <Card className="overflow-hidden border-border/70">
      <CardHeader className="flex flex-row items-center justify-between gap-3 border-b border-border/60 bg-muted/15">
        <div>
          <CardTitle className="text-base">{title}</CardTitle>
          <CardDescription>{count} 条命中</CardDescription>
        </div>
        <Badge variant="secondary" className="rounded-full px-3">{count}</Badge>
      </CardHeader>
      <CardContent className="space-y-3 pt-4">
        {count === 0 ? <p className="text-sm text-muted-foreground">{emptyText}</p> : children}
      </CardContent>
    </Card>
  );
}

function SearchResultCard({
  title,
  description,
  meta,
  value,
  onOpen,
}: {
  title: string;
  description: string;
  meta: ReactNode;
  value: ReactNode;
  onOpen: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onOpen}
      className="w-full rounded-2xl border border-border/70 bg-background/70 px-4 py-3 text-left transition-all duration-200 hover:-translate-y-0.5 hover:border-primary/30 hover:bg-primary/5"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="space-y-1">
          <p className="font-medium">{title}</p>
          <p className="text-sm text-muted-foreground">{description}</p>
          <p className="text-xs text-muted-foreground">{meta}</p>
        </div>
        <div className="flex shrink-0 items-center gap-3 text-sm text-muted-foreground">
          <div>{value}</div>
          <ArrowRight className="h-4 w-4" />
        </div>
      </div>
    </button>
  );
}

function SettingsCategoryPanel({
  title,
  description,
  settings,
  highlightedTarget,
  savingSettingKey,
  onSave,
}: {
  title: string;
  description: string;
  settings: SettingItem[];
  highlightedTarget?: string;
  savingSettingKey: string | null;
  onSave: (setting: SettingItem, value: unknown) => Promise<void>;
}) {
  return (
    <Card className="overflow-hidden border-border/70 shadow-[0_16px_50px_-42px_color-mix(in_oklab,var(--foreground)_42%,transparent)]">
      <CardHeader className="border-b border-border/60 bg-muted/20">
        <CardTitle>{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {settings.length === 0 ? (
          <p className="text-sm text-muted-foreground">当前筛选下没有设置项。</p>
        ) : (
          settings.map((setting) => (
            <SettingRow
              key={setting.key}
              setting={setting}
              highlighted={highlightedTarget === `setting-${setting.key}`}
              saving={savingSettingKey === setting.key}
              onSave={onSave}
            />
          ))
        )}
      </CardContent>
    </Card>
  );
}

function SettingRow({
  setting,
  highlighted,
  saving,
  onSave,
}: {
  setting: SettingItem;
  highlighted: boolean;
  saving: boolean;
  onSave: (setting: SettingItem, value: unknown) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(toTextValue(setting.value));
  const isDirty = draft !== toTextValue(setting.value);

  useEffect(() => {
    setDraft(toTextValue(setting.value));
  }, [setting.value]);

  const commit = async () => {
    const nextValue = setting.control === 'number' ? (draft.trim() ? Number(draft) : null) : draft;
    await onSave(setting, nextValue);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    void commit();
  };

  return (
    <div
      id={`setting-${setting.key}`}
      className={cn(
        'scroll-mt-32 rounded-2xl border border-border/70 bg-card p-4 transition-all duration-300',
        highlighted && 'border-primary/40 bg-primary/5 shadow-[0_18px_50px_-36px_color-mix(in_oklab,var(--primary)_80%,transparent)] ring-1 ring-primary/20',
        isDirty && 'border-accent/45',
      )}
    >
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <p className="font-medium">{setting.label}</p>
            <Badge variant="outline" className="rounded-full font-mono">{setting.key}</Badge>
            {isDirty && !saving && (
              <Badge variant="outline" className="rounded-full border-accent/40 bg-accent/10 text-accent-foreground">
                Pending
              </Badge>
            )}
            {saving && (
              <Badge variant="secondary" className="rounded-full">
                <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                保存中
              </Badge>
            )}
          </div>
          <p className="max-w-2xl text-sm text-muted-foreground">{setting.description}</p>
          <div className="flex flex-wrap gap-2 text-xs text-muted-foreground [&>span]:rounded-full [&>span]:bg-muted [&>span]:px-2.5 [&>span]:py-1">
            <span>当前值: {toDisplayValue(setting.value)}</span>
            <span>生效值: {toDisplayValue(setting.effective_value)}</span>
            <span>默认值: {toDisplayValue(setting.default_value)}</span>
          </div>
        </div>

        <div className="w-full max-w-sm">
          {setting.control === 'toggle' ? (
            <div className="flex items-center justify-between rounded-2xl border border-border/70 bg-background/70 px-3 py-2.5">
              <div>
                <p className="text-sm font-medium">立即生效</p>
                <p className="text-xs text-muted-foreground">切换后自动保存</p>
              </div>
              <Switch
                checked={toBooleanValue(setting.effective_value)}
                onCheckedChange={(checked) => void onSave(setting, checked)}
                disabled={saving}
              />
            </div>
          ) : (
            <div className="space-y-2">
              <Input
              value={draft}
              type={setting.control === 'number' ? 'number' : 'text'}
              onChange={(event) => setDraft(event.target.value)}
              onBlur={() => void commit()}
              onKeyDown={onKeyDown}
              className={cn(
                'h-11 rounded-xl border-border/70 bg-background/80',
                isDirty && 'border-accent/50 ring-1 ring-accent/15',
              )}
              placeholder={
                setting.validation.allow_empty ? `默认值: ${toDisplayValue(setting.default_value)}` : ''
              }
              />
              <p className="text-[11px] text-muted-foreground">
                {t('settings.inlineSaveHint', 'Press Enter or blur to save.')}
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function ChatProvidersPanel({
  providers,
  query,
  saving,
  onAddProvider,
  onChange,
  onSave,
}: {
  providers: CloudProviderDraft[];
  query: string;
  saving: boolean;
  onAddProvider: () => void;
  onChange: (providers: CloudProviderDraft[]) => void;
  onSave: () => void;
}) {
  const filteredProviders = providers.filter((provider) =>
    matchesSearch(query, [
      provider.id,
      provider.name,
      provider.api_base,
      provider.api_key_env,
      provider.models.map((model) => `${model.id} ${model.display_name} ${model.remote_model}`),
    ]),
  );

  return (
    <Card className="overflow-hidden border-border/70 shadow-[0_16px_50px_-42px_color-mix(in_oklab,var(--foreground)_42%,transparent)]">
      <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <CardTitle>聊天提供商</CardTitle>
          <CardDescription>结构化管理 `chat_model_providers`，不再直接编辑原始 JSON。</CardDescription>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={onAddProvider}>
            <Plus className="mr-2 h-4 w-4" />
            新增提供商
          </Button>
          <Button onClick={onSave} disabled={saving}>
            {saving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            保存提供商
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {filteredProviders.length === 0 ? (
          <p className="text-sm text-muted-foreground">当前筛选下没有提供商配置。</p>
        ) : (
          filteredProviders.map((provider, providerIndex) => (
            <Card key={`${provider.id}-${providerIndex}`} className="border-dashed">
              <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                <div>
                  <CardTitle className="text-base">{provider.name || provider.id || `提供商 ${providerIndex + 1}`}</CardTitle>
                  <CardDescription>{provider.api_base || '未配置 api_base'}</CardDescription>
                </div>
                <div className="flex items-center gap-2">
                  <Badge variant="outline">API Key {maskSecret(provider.api_key)}</Badge>
                  <Button variant="ghost" size="icon" onClick={() => onChange(providers.filter((_, index) => index !== providerIndex))}>
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid gap-4 lg:grid-cols-2">
                  <LabeledInput label="Provider ID" value={provider.id} onChange={(value) => onChange(providers.map((item, index) => index === providerIndex ? { ...item, id: value } : item))} />
                  <LabeledInput label="Display Name" value={provider.name} onChange={(value) => onChange(providers.map((item, index) => index === providerIndex ? { ...item, name: value } : item))} />
                  <LabeledInput label="API Base" value={provider.api_base} onChange={(value) => onChange(providers.map((item, index) => index === providerIndex ? { ...item, api_base: value } : item))} />
                  <LabeledInput label="API Key Env" value={provider.api_key_env} onChange={(value) => onChange(providers.map((item, index) => index === providerIndex ? { ...item, api_key_env: value } : item))} placeholder="OPENAI_API_KEY" />
                  <LabeledInput label="API Key" value={provider.api_key} onChange={(value) => onChange(providers.map((item, index) => index === providerIndex ? { ...item, api_key: value } : item))} type="password" placeholder="sk-..." />
                </div>

                <Separator />

                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-sm font-medium">模型映射</p>
                      <p className="text-xs text-muted-foreground">每个提供商至少保留一个模型。</p>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() =>
                        onChange(
                          providers.map((item, index) =>
                            index === providerIndex
                              ? { ...item, models: [...item.models, { id: '', display_name: '', remote_model: '' }] }
                              : item,
                          ),
                        )
                      }
                    >
                      <Plus className="mr-2 h-3.5 w-3.5" />
                      新增模型
                    </Button>
                  </div>

                  {provider.models.map((model, modelIndex) => (
                    <div key={`${model.id}-${modelIndex}`} className="grid gap-3 rounded-xl border p-3 lg:grid-cols-[1fr_1fr_1fr_auto]">
                      <LabeledInput label="Model ID" value={model.id} onChange={(value) => onChange(providers.map((item, index) => index === providerIndex ? { ...item, models: item.models.map((entry, entryIndex) => entryIndex === modelIndex ? { ...entry, id: value } : entry) } : item))} />
                      <LabeledInput label="Display Name" value={model.display_name} onChange={(value) => onChange(providers.map((item, index) => index === providerIndex ? { ...item, models: item.models.map((entry, entryIndex) => entryIndex === modelIndex ? { ...entry, display_name: value } : entry) } : item))} />
                      <LabeledInput label="Remote Model" value={model.remote_model} onChange={(value) => onChange(providers.map((item, index) => index === providerIndex ? { ...item, models: item.models.map((entry, entryIndex) => entryIndex === modelIndex ? { ...entry, remote_model: value } : entry) } : item))} />
                      <div className="flex items-end">
                        <Button variant="ghost" size="icon" onClick={() => onChange(providers.map((item, index) => index === providerIndex ? { ...item, models: item.models.filter((_, entryIndex) => entryIndex !== modelIndex) } : item))}>
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          ))
        )}
      </CardContent>
    </Card>
  );
}

function LabeledInput({
  label,
  value,
  onChange,
  placeholder,
  type = 'text',
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: string;
}) {
  return (
    <div className="grid gap-2">
      <Label>{label}</Label>
      <Input value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} type={type} />
    </div>
  );
}

function ModelCatalogPanel({
  models,
  totalModels,
  installedModelCount,
  downloadingModelCount,
  highlightedTarget,
  backendFilter,
  onBackendFilterChange,
  statusFilter,
  onStatusFilterChange,
  backendOptions,
  busyModelId,
  deletingModelId,
  onRefresh,
  onCreateModel,
  onEditModel,
  onDeleteModel,
  onDownloadModel,
}: {
  models: ModelCatalogItem[];
  totalModels: number;
  installedModelCount: number;
  downloadingModelCount: number;
  highlightedTarget: string;
  backendFilter: string;
  onBackendFilterChange: (value: string) => void;
  statusFilter: StatusFilter;
  onStatusFilterChange: (value: StatusFilter) => void;
  backendOptions: string[];
  busyModelId: string | null;
  deletingModelId: string | null;
  onRefresh: () => void;
  onCreateModel: () => void;
  onEditModel: (model: ModelCatalogItem) => void;
  onDeleteModel: (id: string) => void;
  onDownloadModel: (model: ModelCatalogItem) => void;
}) {
  return (
    <Card>
      <CardHeader className="flex flex-col gap-4 border-b border-border/60 bg-muted/15 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <CardTitle>模型目录</CardTitle>
          <CardDescription>模型管理已经收敛到 Settings，不再分散在 Hub 和旧设置页里。</CardDescription>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={onRefresh}>
            <RefreshCw className="mr-2 h-4 w-4" />
            刷新目录
          </Button>
          <Button onClick={onCreateModel}>
            <Plus className="mr-2 h-4 w-4" />
            新增模型
          </Button>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-4 md:grid-cols-3">
          <CompactStatCard title="总模型数" value={totalModels} />
          <CompactStatCard title="已下载" value={installedModelCount} />
          <CompactStatCard title="下载中" value={downloadingModelCount} />
        </div>

        <div className="grid gap-3 md:grid-cols-2">
          <Select value={backendFilter} onValueChange={onBackendFilterChange}>
            <SelectTrigger><SelectValue placeholder="筛选 backend" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="all">所有 backend</SelectItem>
              {backendOptions.map((backendId) => <SelectItem key={backendId} value={backendId}>{backendId}</SelectItem>)}
            </SelectContent>
          </Select>

          <Select value={statusFilter} onValueChange={(value) => onStatusFilterChange(value as StatusFilter)}>
            <SelectTrigger><SelectValue placeholder="筛选状态" /></SelectTrigger>
            <SelectContent>
              <SelectItem value="all">所有状态</SelectItem>
              <SelectItem value="downloaded">已下载</SelectItem>
              <SelectItem value="pending">下载中</SelectItem>
              <SelectItem value="not_downloaded">未下载</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-3">
          {models.length === 0 ? (
            <p className="text-sm text-muted-foreground">当前筛选下没有模型条目。</p>
          ) : (
            models.map((model) => {
              const status = modelStatusOf(model);
              return (
                <div
                  key={model.id}
                  id={`model-${model.id}`}
                  className={cn(
                    'scroll-mt-32 rounded-2xl border px-4 py-4 transition-all duration-300',
                    highlightedTarget === `model-${model.id}` && 'border-primary/40 bg-primary/5 shadow-[0_18px_50px_-36px_color-mix(in_oklab,var(--primary)_80%,transparent)] ring-1 ring-primary/20',
                  )}
                >
                  <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                    <div className="space-y-2">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="font-medium">{model.display_name}</p>
                        {statusBadge(status)}
                      </div>
                      <p className="font-mono text-xs text-muted-foreground">{model.id}</p>
                      <p className="text-sm text-muted-foreground">{model.repo_id} / {model.filename}</p>
                      <div className="flex flex-wrap gap-2">
                        {model.backend_ids.map((backendId) => <Badge key={backendId} variant="outline">{backendId}</Badge>)}
                      </div>
                      <p className="text-xs text-muted-foreground">
                        本地路径: {model.local_path ?? '未下载'} · 上次下载: {formatDate(model.last_downloaded_at)}
                      </p>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <Button variant="outline" onClick={() => onDownloadModel(model)} disabled={busyModelId === model.id || status === 'downloaded'}>
                        {busyModelId === model.id && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                        <Download className="mr-2 h-4 w-4" />
                        {status === 'downloaded' ? '已下载' : '下载'}
                      </Button>
                      <Button variant="outline" onClick={() => onEditModel(model)}>
                        <Pencil className="mr-2 h-4 w-4" />
                        编辑
                      </Button>
                      <AlertDialog>
                        <AlertDialogTrigger asChild>
                          <Button variant="destructive" disabled={deletingModelId === model.id}>
                            {deletingModelId === model.id && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                            <Trash2 className="mr-2 h-4 w-4" />
                            删除
                          </Button>
                        </AlertDialogTrigger>
                        <AlertDialogContent>
                          <AlertDialogHeader>
                            <AlertDialogTitle>删除模型条目？</AlertDialogTitle>
                            <AlertDialogDescription>这会把 {model.display_name} 从模型目录中移除。</AlertDialogDescription>
                          </AlertDialogHeader>
                          <AlertDialogFooter>
                            <AlertDialogCancel>取消</AlertDialogCancel>
                            <AlertDialogAction variant="destructive" onClick={() => onDeleteModel(model.id)}>删除</AlertDialogAction>
                          </AlertDialogFooter>
                        </AlertDialogContent>
                      </AlertDialog>
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function CompactStatCard({ title, value }: { title: string; value: number }) {
  return (
    <div className="rounded-2xl border border-border/70 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--muted)_72%,transparent),color-mix(in_oklab,var(--card)_88%,transparent))] p-4 shadow-sm">
      <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">{title}</p>
      <p className="mt-2 text-3xl font-semibold">{value}</p>
    </div>
  );
}

function BackendsPanel({
  backends,
  systemBackendMap,
  highlightedTarget,
  activeBackendId,
  onRefresh,
  onCheckStatus,
  onDownloadLib,
  onReloadLib,
}: {
  backends: BackendListItem[];
  systemBackendMap: Map<string, SettingsSystemBackend>;
  highlightedTarget: string;
  activeBackendId: string | null;
  onRefresh: () => void;
  onCheckStatus: (backendId: string) => void;
  onDownloadLib: (backendId: string) => void;
  onReloadLib: (backendId: string) => void;
}) {
  return (
    <Card className="overflow-hidden border-border/70 shadow-[0_16px_50px_-42px_color-mix(in_oklab,var(--foreground)_42%,transparent)]">
      <CardHeader className="flex flex-col gap-4 border-b border-border/60 bg-muted/15 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <CardTitle>后端管理</CardTitle>
          <CardDescription>查看 endpoint 配置、实际运行状态和 worker 生效值。</CardDescription>
        </div>
        <Button variant="outline" onClick={onRefresh}>
          <RefreshCw className="mr-2 h-4 w-4" />
          刷新状态
        </Button>
      </CardHeader>
      <CardContent className="space-y-4">
        {backends.length === 0 ? (
          <p className="text-sm text-muted-foreground">没有可展示的 backend。</p>
        ) : (
          backends.map((backend) => {
            const systemBackend = systemBackendMap.get(backend.backend);
            const isActive = activeBackendId === backend.backend;
            return (
              <div
                key={backend.backend}
                id={`backend-${backend.backend}`}
                className={cn(
                  'scroll-mt-32 rounded-2xl border px-4 py-4 transition-all duration-300',
                  highlightedTarget === `backend-${backend.backend}` && 'border-primary/40 bg-primary/5 shadow-[0_18px_50px_-36px_color-mix(in_oklab,var(--primary)_80%,transparent)] ring-1 ring-primary/20',
                )}
              >
                <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                  <div className="space-y-2">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="font-medium">{backend.backend}</p>
                      <Badge variant={backend.status === 'ready' ? 'default' : 'outline'}>{backend.status}</Badge>
                    </div>
                    <p className="text-sm text-muted-foreground">Endpoint: {systemBackend?.endpoint ?? '未配置'}</p>
                    <p className="text-sm text-muted-foreground">Runtime: {systemBackend?.runtime_status ?? backend.status} · Workers: {systemBackend?.effective_workers ?? 'n/a'}</p>
                    <p className="text-xs text-muted-foreground">Worker Key: {systemBackend?.worker_setting_key ?? '无'} · Configured: {systemBackend?.configured_workers ?? '默认'}</p>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <Button variant="outline" onClick={() => onCheckStatus(backend.backend)} disabled={isActive}>
                      {isActive && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                      检查状态
                    </Button>
                    <Button variant="outline" onClick={() => onDownloadLib(backend.backend)} disabled={isActive}>
                      <Download className="mr-2 h-4 w-4" />
                      下载运行库
                    </Button>
                    <Button onClick={() => onReloadLib(backend.backend)} disabled={isActive}>
                      <RefreshCw className="mr-2 h-4 w-4" />
                      重载
                    </Button>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </CardContent>
    </Card>
  );
}

function SystemInfoPanel({ system }: { system: SettingsSystemResponse | null }) {
  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>系统信息</CardTitle>
          <CardDescription>这些值来自 server 启动配置，只读展示，不允许在 UI 中修改。</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-2">
          <ReadonlyFact label="Bind Address" value={system?.bind_address ?? '加载中'} icon={Database} />
          <ReadonlyFact label="Transport Mode" value={system?.transport_mode ?? '加载中'} icon={Layers2} />
          <ReadonlyFact label="Swagger" value={system?.swagger_enabled ? '开启' : '关闭'} icon={Info} />
          <ReadonlyFact label="Admin Token" value={system?.admin_token_enabled ? '已配置' : '未配置'} icon={Server} />
          <ReadonlyFact label="CORS" value={system?.cors_configured ? '已配置' : 'Allow All'} icon={Cloud} />
          <ReadonlyFact label="Session State Dir" value={system?.session_state_dir ?? '加载中'} icon={Boxes} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Backend Facts</CardTitle>
          <CardDescription>从运行时读到的 backend 配置摘要。</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {(system?.backends ?? []).map((backend) => (
            <div key={backend.backend} className="rounded-xl border px-4 py-3">
              <div className="flex flex-wrap items-center gap-2">
                <p className="font-medium">{backend.backend}</p>
                <Badge variant={backend.runtime_status === 'ready' ? 'default' : 'outline'}>
                  {backend.runtime_status}
                </Badge>
              </div>
              <p className="mt-2 text-sm text-muted-foreground">Endpoint: {backend.endpoint ?? '未配置'}</p>
              <p className="text-sm text-muted-foreground">Worker Key: {backend.worker_setting_key ?? '无'} · Effective Workers: {backend.effective_workers ?? 'n/a'}</p>
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}

function ReadonlyFact({
  label,
  value,
  icon: Icon,
}: {
  label: string;
  value: string;
  icon: typeof Info;
}) {
  return (
    <div className="rounded-2xl border border-border/70 bg-background/70 p-4 shadow-sm">
      <div className="flex items-center gap-2 text-xs uppercase tracking-[0.2em] text-muted-foreground">
        <Icon className="h-3.5 w-3.5" />
        {label}
      </div>
      <p className="mt-3 break-all text-sm">{value}</p>
    </div>
  );
}
