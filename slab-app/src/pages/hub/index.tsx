import { useState } from 'react';
import { Link } from 'react-router-dom';
import { ArrowRight, Boxes, Loader2, Search, Settings2 } from 'lucide-react';
import { toast } from 'sonner';

import api, { getErrorMessage } from '@/lib/api';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

type AvailableFilesResponse = {
  repo_id?: string;
  files?: string[];
};

export default function Hub() {
  const [repoId, setRepoId] = useState('bartowski/Qwen2.5-0.5B-Instruct-GGUF');
  const [availableFiles, setAvailableFiles] = useState<string[]>([]);
  const [resolvedRepoId, setResolvedRepoId] = useState('');

  const listAvailableMutation = api.useMutation('get', '/v1/models/available');

  const loadFiles = async () => {
    const trimmed = repoId.trim();
    if (!trimmed) {
      toast.error('请输入 Hugging Face repo id');
      return;
    }

    try {
      const response = (await listAvailableMutation.mutateAsync({
        params: {
          query: { repo_id: trimmed },
        },
      })) as AvailableFilesResponse;
      setAvailableFiles(Array.isArray(response.files) ? response.files : []);
      setResolvedRepoId(response.repo_id ?? trimmed);
    } catch (error) {
      toast.error(getErrorMessage(error));
    }
  };

  return (
    <div className="container mx-auto max-w-6xl space-y-6 px-4 py-8">
      <div className="grid gap-6 lg:grid-cols-[1.2fr_0.8fr]">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Search className="h-5 w-5" />
              Hub 资源发现
            </CardTitle>
            <CardDescription>
              Hub 现在只负责资源发现与 repo 文件浏览。模型目录、运行时设置和后端管理已经迁移到新的 Settings 控制台。
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-2">
              <Label>Hugging Face Repo ID</Label>
              <Input
                value={repoId}
                onChange={(event) => setRepoId(event.target.value)}
                placeholder="bartowski/Qwen2.5-0.5B-Instruct-GGUF"
              />
            </div>
            <div className="flex gap-2">
              <Button onClick={() => void loadFiles()} disabled={listAvailableMutation.isPending}>
                {listAvailableMutation.isPending && (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                )}
                浏览文件
              </Button>
              <Button variant="outline" asChild>
                <Link to="/settings?section=models">
                  打开模型目录
                  <ArrowRight className="ml-2 h-4 w-4" />
                </Link>
              </Button>
            </div>

            <div className="rounded-2xl border p-4">
              <div className="flex items-center justify-between gap-2">
                <div>
                  <p className="font-medium">{resolvedRepoId || '尚未查询 repo'}</p>
                  <p className="text-sm text-muted-foreground">
                    {availableFiles.length > 0
                      ? `${availableFiles.length} 个文件`
                      : '查询后会在这里展示可用文件列表'}
                  </p>
                </div>
              </div>

              <div className="mt-4 max-h-[420px] space-y-2 overflow-y-auto pr-1">
                {availableFiles.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    选择一个 repo 后，Hub 会列出可用文件，方便你回到 Settings 新建模型条目。
                  </p>
                ) : (
                  availableFiles.map((file) => (
                    <div
                      key={file}
                      className="rounded-xl border bg-muted/20 px-3 py-2 font-mono text-xs"
                    >
                      {file}
                    </div>
                  ))
                )}
              </div>
            </div>
          </CardContent>
        </Card>

        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Settings2 className="h-5 w-5" />
                Settings 控制台
              </CardTitle>
              <CardDescription>
                新的 Settings 页面是统一入口，整合了搜索、设置项展示、模型目录和后端管理。
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="rounded-2xl border p-4">
                <p className="font-medium">现在可以在 Settings 完成：</p>
                <ul className="mt-3 list-disc space-y-2 pl-5 text-sm text-muted-foreground">
                  <li>搜索设置项、模型和后端</li>
                  <li>编辑运行时设置与 Diffusion 配置</li>
                  <li>管理模型目录、下载模型</li>
                  <li>查看后端状态、下载运行库并重载</li>
                </ul>
              </div>
              <Button className="w-full" asChild>
                <Link to="/settings">
                  打开 Settings 控制台
                  <ArrowRight className="ml-2 h-4 w-4" />
                </Link>
              </Button>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Boxes className="h-5 w-5" />
                使用建议
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-3 text-sm text-muted-foreground">
              <p>1. 在 Hub 输入 repo id，确认目标模型文件名。</p>
              <p>2. 跳转到 Settings 的“模型目录”，把 repo id 和 filename 保存成模型条目。</p>
              <p>3. 回到 Settings 里直接下载、搜索和管理这些模型。</p>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
