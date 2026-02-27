import { useState } from 'react';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Spinner } from '@/components/ui/spinner';
import { toast } from 'sonner';
import { Loader2 } from 'lucide-react';
import api from '@/lib/api';

export default function Image() {
  const [prompt, setPrompt] = useState('');
  const [model, setModel] = useState('');
  const [numImages, setNumImages] = useState('1');
  const [size, setSize] = useState('512x512');
  const [taskId, setTaskId] = useState<string | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [images, setImages] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  // API mutations
  const generateImageMutation = api.useMutation('post', '/v1/images/generations');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');
  const getTaskResultMutation = api.useMutation('get', '/v1/tasks/{id}/result');

  const handleGenerate = async () => {
    if (!prompt) {
      setError('请输入提示词');
      return;
    }

    if (!model) {
      setError('请选择模型');
      return;
    }

    setError(null);
    setImages([]);
    setIsGenerating(true);

    try {
      const data = await generateImageMutation.mutateAsync({
        body: {
          model,
          prompt,
          n: parseInt(numImages),
          size
        }
      }) as { task_id: string };

      setTaskId(data.task_id);
      setIsGenerating(false);
      setIsProcessing(true);
      
      // 轮询任务状态
      pollTaskStatus(data.task_id);
    } catch (err) {
      setError('生成失败: ' + (err instanceof Error ? err.message : '未知错误'));
      setIsGenerating(false);
      toast.error('生成失败');
    }
  };

  const pollTaskStatus = async (id: string) => {
    try {
      const task = await getTaskMutation.mutateAsync({
        params: {
          path: { id }
        }
      }) as { status: string };
      
      if (task.status === 'succeeded') {
        // 获取生成结果
        const result = await getTaskResultMutation.mutateAsync({
          params: {
            path: { id }
          }
        }) as any;

        // 假设结果包含图像的 base64 编码
        if (result.images && Array.isArray(result.images)) {
          setImages(result.images);
        } else if (result.image) {
          setImages([result.image]);
        } else {
          setImages([result]);
        }
        setIsProcessing(false);
        toast.success('图像生成成功');
      } else if (task.status === 'failed') {
        setError('生成任务失败');
        setIsProcessing(false);
        toast.error('生成任务失败');
      } else {
        // 继续轮询
        setTimeout(() => pollTaskStatus(id), 2000);
      }
    } catch (err) {
      setError('获取任务状态失败');
      setIsProcessing(false);
      toast.error('获取任务状态失败');
    }
  };

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <div className="text-center space-y-4">
        <h1 className="text-3xl font-bold text-center">图像生成</h1>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          输入提示词，系统将生成相应的图像
        </p>
      </div>

      <Card className="max-w-2xl mx-auto">
        <CardHeader>
          <CardTitle>生成设置</CardTitle>
          <CardDescription>配置图像生成参数</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {error && (
            <Alert variant="destructive">
              <AlertTitle>错误</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}

          <div className="space-y-2">
            <Label htmlFor="prompt">提示词</Label>
            <Textarea
              id="prompt"
              placeholder="描述你想要生成的图像..."
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              disabled={isGenerating || isProcessing}
              rows={4}
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label htmlFor="model">模型</Label>
              <Select value={model} onValueChange={setModel} disabled={isGenerating || isProcessing}>
                <SelectTrigger id="model">
                  <SelectValue placeholder="选择模型" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="stable-diffusion">Stable Diffusion</SelectItem>
                  <SelectItem value="dall-e">DALL-E</SelectItem>
                  <SelectItem value="midjourney">Midjourney</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label htmlFor="numImages">生成数量</Label>
              <Select value={numImages} onValueChange={setNumImages} disabled={isGenerating || isProcessing}>
                <SelectTrigger id="numImages">
                  <SelectValue placeholder="选择数量" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">1</SelectItem>
                  <SelectItem value="2">2</SelectItem>
                  <SelectItem value="4">4</SelectItem>
                  <SelectItem value="8">8</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="size">图像尺寸</Label>
            <Select value={size} onValueChange={setSize} disabled={isGenerating || isProcessing}>
              <SelectTrigger id="size">
                <SelectValue placeholder="选择尺寸" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="256x256">256x256</SelectItem>
                <SelectItem value="512x512">512x512</SelectItem>
                <SelectItem value="1024x1024">1024x1024</SelectItem>
                <SelectItem value="1024x1536">1024x1536</SelectItem>
                <SelectItem value="1536x1024">1536x1024</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {(isGenerating || isProcessing) && (
            <div className="flex flex-col items-center space-y-4">
              <Spinner className="h-8 w-8" />
              <p>{isGenerating ? '正在提交请求...' : '正在生成图像，请稍候...'}</p>
              {taskId && (
                <p className="text-xs text-muted-foreground">任务 ID: {taskId}</p>
              )}
            </div>
          )}
        </CardContent>
        <CardFooter className="flex justify-end">
          <Button
            onClick={handleGenerate}
            disabled={!prompt || !model || isGenerating || isProcessing}
          >
            {(isGenerating || isProcessing) ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                生成中...
              </>
            ) : (
              '生成图像'
            )}
          </Button>
        </CardFooter>
      </Card>

      {images.length > 0 && (
        <Card className="max-w-4xl mx-auto">
          <CardHeader>
            <CardTitle>生成结果</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
              {images.map((image, index) => (
                <div key={index} className="space-y-2">
                  <div className="aspect-square rounded-md overflow-hidden border">
                    {typeof image === 'string' && image.startsWith('data:image') ? (
                      <img src={image} alt={`Generated image ${index + 1}`} className="w-full h-full object-cover" />
                    ) : (
                      <div className="w-full h-full flex items-center justify-center bg-muted">
                        <p>无法显示图像</p>
                      </div>
                    )}
                  </div>
                  <Button
                    variant="secondary"
                    size="sm"
                    className="w-full"
                    onClick={() => {
                      if (typeof image === 'string' && image.startsWith('data:image')) {
                        // 创建临时链接并下载
                        const link = document.createElement('a');
                        link.href = image;
                        link.download = `generated-image-${index + 1}.png`;
                        link.click();
                      }
                    }}
                  >
                    下载
                  </Button>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}