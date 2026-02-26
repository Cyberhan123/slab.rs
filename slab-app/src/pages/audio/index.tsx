import { useState } from 'react';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Progress } from '@/components/ui/progress';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Spinner } from '@/components/ui/spinner';
import api from '@/lib/api';

export default function Audio() {
  const [file, setFile] = useState<File | null>(null);
  const [isUploading, setIsUploading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState(0);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [isProcessing, setIsProcessing] = useState(false);
  const [transcription, setTranscription] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // API mutations - 使用与 hub 页面相同的模式
  const transcribeMutation = api.useMutation('post', '/v1/audio/transcriptions');
  const getTaskMutation = api.useMutation('post', '/v1/tasks/{id}');
  const getTaskResultMutation = api.useMutation('post', '/v1/tasks/{id}/result');

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files[0]) {
      setFile(e.target.files[0]);
      setError(null);
    }
  };

  const handleTranscribe = async () => {
    if (!file) {
      setError('请选择一个音频或视频文件');
      return;
    }

    setIsUploading(true);
    setError(null);
    setTranscription(null);

    try {
      // 读取文件内容
      const reader = new FileReader();
      reader.onprogress = (e) => {
        if (e.lengthComputable) {
          setUploadProgress(Math.round((e.loaded / e.total) * 100));
        }
      };

      reader.onload = async (e) => {
        const arrayBuffer = e.target?.result as ArrayBuffer;
        const uint8Array = new Uint8Array(arrayBuffer);
        
        // 将 Uint8Array 转换为 number[]
        const bodyArray = Array.from(uint8Array);
        
        // 调用 API - 使用与 hub 页面相同的 mutation 模式
        const data = await transcribeMutation.mutateAsync({
          body: bodyArray
        }) as { task_id: string };

        setTaskId(data.task_id);
        setIsUploading(false);
        setIsProcessing(true);
        
        // 轮询任务状态
        pollTaskStatus(data.task_id);
      };

      reader.onerror = () => {
        throw new Error('文件读取失败');
      };

      reader.readAsArrayBuffer(file);
    } catch (err) {
      setError('转录失败: ' + (err instanceof Error ? err.message : '未知错误'));
      setIsUploading(false);
    }
  };

  const pollTaskStatus = async (id: string) => {
    try {
      // 调用 API - 使用与 hub 页面相同的 mutation 模式
      const task = await getTaskMutation.mutateAsync({
        params: {
          path: { id }
        }
      }) as { status: string };

      if (task.status === 'completed') {
        // 获取转录结果 - 使用与 hub 页面相同的 mutation 模式
        const result = await getTaskResultMutation.mutateAsync({
          params: {
            path: { id }
          }
        }) as any;

        setTranscription(result.text || result.transcription || JSON.stringify(result));
        setIsProcessing(false);
      } else if (task.status === 'failed') {
        setError('转录任务失败');
        setIsProcessing(false);
      } else {
        // 继续轮询
        setTimeout(() => pollTaskStatus(id), 2000);
      }
    } catch (err) {
      setError('获取任务状态失败');
      setIsProcessing(false);
    }
  };

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <div className="text-center space-y-4">
        <h1 className="text-3xl font-bold text-center">音频转录</h1>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          上传音频或视频文件，系统将自动转录为文本
        </p>
      </div>

      <Card className="max-w-2xl mx-auto">
        <CardHeader>
          <CardTitle>上传文件</CardTitle>
          <CardDescription>支持音频和视频文件格式</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {error && (
            <Alert variant="destructive">
              <AlertTitle>错误</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}

          <div className="space-y-2">
            <Label htmlFor="file">选择文件</Label>
            <Input
              id="file"
              type="file"
              accept="audio/*,video/*"
              onChange={handleFileChange}
              disabled={isUploading || isProcessing}
            />
            {file && (
              <p className="text-sm text-muted-foreground">
                已选择: {file.name} ({(file.size / 1024 / 1024).toFixed(2)} MB)
              </p>
            )}
          </div>

          {isUploading && (
            <div className="space-y-2">
              <Progress value={uploadProgress} />
              <p className="text-sm text-center">上传中... {uploadProgress}%</p>
            </div>
          )}

          {isProcessing && (
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
            disabled={!file || isUploading || isProcessing}
          >
            {isUploading ? '上传中...' : isProcessing ? '处理中...' : '开始转录'}
          </Button>
        </CardFooter>
      </Card>

      {transcription && (
        <Card className="max-w-2xl mx-auto">
          <CardHeader>
            <CardTitle>转录结果</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <div className="p-4 border rounded-md bg-muted/50">
                <p className="whitespace-pre-wrap">{transcription}</p>
              </div>
              <Button
                variant="secondary"
                onClick={() => {
                  navigator.clipboard.writeText(transcription);
                }}
              >
                复制结果
              </Button>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}