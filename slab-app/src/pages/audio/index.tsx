import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Spinner } from '@/components/ui/spinner';
import { toast } from 'sonner';
import useFile, { SelectedFile } from '@/hooks/use-file';
import useTranscribe from './hooks/use-transcribe';
import useIsTauri from '@/hooks/use-tauri';


export default function Audio() {
  const navigate = useNavigate();
  const isTauri = useIsTauri();
  // file object or string path
  const [file, setFile] = useState<SelectedFile | null>(null);

  const [taskId, setTaskId] = useState<string | null>(null);
  const { handleFile } = useFile();

  const transcribe = useTranscribe();

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile = await handleFile(e);

    if (selectedFile) {
      setFile(selectedFile);
    }
  };

  const handleTranscribe = async () => {
    if (!file) {
      toast.error('请先选择一个文件');
      return
    }

    try {
      const result = await transcribe.handleTranscribe(file.file);
      setTaskId(result.task_id);

      // Show success toast with navigation option
      toast.success('转录任务已创建', {
        description: `任务 ID: ${result.task_id}`,
        action: {
          label: '查看任务',
          onClick: () => navigate('/task')
        }
      });
    } catch (err: any) {
      toast.error('创建转录任务失败', {
        description: err?.message || err?.error || '未知错误'
      });
    }
  };

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <div className="text-center space-y-4">
        <h1 className="text-3xl font-bold text-center">音频转录</h1>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          { isTauri ? "选择音视频文件路径" : "Web端暂不支持上传音频或视频文件" }
        </p>
      </div>

      <Card className="max-w-2xl mx-auto">
        <CardHeader>
          <CardTitle>上传文件</CardTitle>
          <CardDescription>支持音频和视频文件格式</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {transcribe?.isError && (
            <Alert variant="destructive">
              <AlertTitle>错误</AlertTitle>
              <AlertDescription>
                {(transcribe?.error as any)?.error || '创建转录任务失败，请重试'}
              </AlertDescription>
            </Alert>
          )}

          <div className="space-y-2">
            <Label htmlFor="file">选择文件</Label>
            <Input
              id="file"
              type="file"
              accept="audio/*,video/*"
              onChange={handleFileChange}
              disabled={transcribe?.isPending}
            />
            {file && (
              <p className="text-sm text-muted-foreground">
                已选择: {file.name}
              </p>
            )}
          </div>

          {transcribe?.isPending && (
            <div className="flex flex-col items-center space-y-4">
              <Spinner className="h-8 w-8" />
              <p>正在处理转录请求，请稍候...</p>
              {taskId && (
                <p className="text-xs text-muted-foreground">任务 ID: {taskId}</p>
              )}
            </div>
          )}
        </CardContent>
        <CardFooter className="flex justify-end">
          <Button
            onClick={handleTranscribe}
            disabled={!file || transcribe?.isPending}
          >
            {transcribe?.isPending ? '处理中...' : '开始转录'}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}