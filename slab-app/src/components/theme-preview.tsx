import * as React from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { Checkbox } from "@/components/ui/checkbox"
import { Switch } from "@/components/ui/switch"
import { Slider } from "@/components/ui/slider"
import { Progress } from "@/components/ui/progress"
import { Separator } from "@/components/ui/separator"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Skeleton } from "@/components/ui/skeleton"
import { Toggle } from "@/components/ui/toggle"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog"
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger } from "@/components/ui/sheet"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Pagination, PaginationContent, PaginationEllipsis, PaginationItem, PaginationLink, PaginationNext, PaginationPrevious } from "@/components/ui/pagination"
import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from "@/components/ui/breadcrumb"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  Bot, User, Send, Sparkles, Wrench, CheckCircle, Code, MessageSquare,
  Palette, Settings, Moon, Sun, Bell, Search, Plus, Trash2, Edit3,
  ChevronDown, Info, AlertTriangle, AlertCircle, Terminal, Bold, Italic,
  Underline, AlignLeft, AlignCenter, AlignRight, Home, FileText, Image,
  Heart, Star, MoreHorizontal, Download, Share2, Eye, Copy, Zap,
} from "lucide-react"

/* ======================================================================
   色块展示
   ====================================================================== */
function ColorSwatch({ name, className }: { name: string; className: string }) {
  return (
    <div className="flex flex-col items-center gap-1.5">
      <div className={`h-14 w-14 rounded-xl border border-border/50 shadow-sm ${className}`} />
      <span className="text-[10px] text-muted-foreground font-mono leading-tight text-center">{name}</span>
    </div>
  )
}

/* ======================================================================
   Section 包装
   ====================================================================== */
function Section({ title, desc, children }: { title: string; desc: string; children: React.ReactNode }) {
  return (
    <section>
      <h2 className="text-xl font-semibold mb-1 text-foreground text-balance">{title}</h2>
      <p className="text-sm text-muted-foreground mb-5">{desc}</p>
      {children}
    </section>
  )
}

/* ======================================================================
   AI 聊天气泡
   ====================================================================== */
function ChatBubbleAI() {
  return (
    <div className="flex items-start gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
        <Bot className="h-4 w-4" />
      </div>
      <div className="rounded-2xl rounded-tl-sm bg-ai-bubble px-4 py-3 text-ai-bubble-foreground max-w-md shadow-sm">
        <p className="text-sm leading-relaxed">
          {"你好！我是你的 AI 助手。我可以帮你完成各种任务，包括代码生成、文本分析和数据处理。"}
        </p>
      </div>
    </div>
  )
}

function ChatBubbleUser() {
  return (
    <div className="flex items-start gap-3 flex-row-reverse">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-secondary text-secondary-foreground">
        <User className="h-4 w-4" />
      </div>
      <div className="rounded-2xl rounded-tr-sm bg-user-bubble px-4 py-3 text-user-bubble-foreground max-w-md shadow-sm">
        <p className="text-sm leading-relaxed">{"帮我写一个带搜索功能的下拉选择器组件"}</p>
      </div>
    </div>
  )
}

function ToolCallCard() {
  return (
    <div className="flex items-start gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-ai-tool text-ai-tool-foreground">
        <Wrench className="h-4 w-4" />
      </div>
      <Card className="max-w-md border-ai-tool/30 bg-ai-tool/10">
        <CardHeader className="pb-2 pt-3 px-4">
          <div className="flex items-center gap-2">
            <Code className="h-4 w-4 text-ai-tool" />
            <CardTitle className="text-sm font-medium text-foreground">{"代码生成工具"}</CardTitle>
            <Badge variant="outline" className="text-[10px] border-success/40 text-success bg-success/10">
              <CheckCircle className="h-3 w-3 mr-1" />
              {"已完成"}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="px-4 pb-3">
          <pre className="text-xs font-mono bg-muted/80 rounded-lg p-3 text-foreground overflow-x-auto">
{`function SearchSelect({ options }) {
  const [query, setQuery] = useState("")
  const filtered = options.filter(
    o => o.label.includes(query)
  )
  return <Dropdown items={filtered} />
}`}
          </pre>
        </CardContent>
      </Card>
    </div>
  )
}

/* ======================================================================
   主展示页
   ====================================================================== */
export function ThemePreview() {
  const [progress, setProgress] = React.useState(66)
  const [sliderVal, setSliderVal] = React.useState([35])

  React.useEffect(() => {
    const t = setInterval(() => setProgress(p => (p >= 100 ? 0 : p + 1)), 80)
    return () => clearInterval(t)
  }, [])

  return (
    <TooltipProvider>
      <div className="min-h-screen bg-background text-foreground">
        {/* ── Header ── */}
        <header className="border-b border-border bg-card/80 backdrop-blur-sm sticky top-0 z-10">
          <div className="max-w-6xl mx-auto px-6 py-4 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-sm">
                <Sparkles className="h-5 w-5" />
              </div>
              <div>
                <h1 className="text-lg font-semibold text-foreground">{"AI Assistant"}</h1>
                <p className="text-xs text-muted-foreground">{"Soft Cyan — 全组件主题预览"}</p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Badge variant="secondary"><Palette className="h-3 w-3 mr-1" />{"主题"}</Badge>
            </div>
          </div>
        </header>

        <main className="max-w-6xl mx-auto px-6 py-8 space-y-12">

          {/* ═══════════════════════════════════════════════
              1. 色彩系统
              ═══════════════════════════════════════════════ */}
          <Section title="色彩系统" desc="所有设计 token 的颜色一览">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <Card>
                <CardHeader className="pb-3">
                  <CardTitle className="text-sm">{"核心色彩"}</CardTitle>
                  <CardDescription>{"primary / secondary / accent / muted / destructive"}</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="flex flex-wrap gap-4">
                    <ColorSwatch name="primary" className="bg-primary" />
                    <ColorSwatch name="secondary" className="bg-secondary" />
                    <ColorSwatch name="accent" className="bg-accent" />
                    <ColorSwatch name="muted" className="bg-muted" />
                    <ColorSwatch name="destructive" className="bg-destructive" />
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardHeader className="pb-3">
                  <CardTitle className="text-sm">{"界面元素"}</CardTitle>
                  <CardDescription>{"background / card / popover / border / input / ring"}</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="flex flex-wrap gap-4">
                    <ColorSwatch name="background" className="bg-background" />
                    <ColorSwatch name="card" className="bg-card" />
                    <ColorSwatch name="popover" className="bg-popover" />
                    <ColorSwatch name="border" className="bg-border" />
                    <ColorSwatch name="input" className="bg-input" />
                    <ColorSwatch name="ring" className="bg-ring" />
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardHeader className="pb-3">
                  <CardTitle className="text-sm">{"AI 聊天专用"}</CardTitle>
                  <CardDescription>{"ai-bubble / user-bubble / ai-tool / success"}</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="flex flex-wrap gap-4">
                    <ColorSwatch name="ai-bubble" className="bg-ai-bubble" />
                    <ColorSwatch name="user-bubble" className="bg-user-bubble" />
                    <ColorSwatch name="ai-tool" className="bg-ai-tool" />
                    <ColorSwatch name="success" className="bg-success" />
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardHeader className="pb-3">
                  <CardTitle className="text-sm">{"侧边栏"}</CardTitle>
                  <CardDescription>{"sidebar / sidebar-primary / sidebar-accent / sidebar-border"}</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="flex flex-wrap gap-4">
                    <ColorSwatch name="sidebar" className="bg-sidebar" />
                    <ColorSwatch name="sb-primary" className="bg-sidebar-primary" />
                    <ColorSwatch name="sb-accent" className="bg-sidebar-accent" />
                    <ColorSwatch name="sb-border" className="bg-sidebar-border" />
                  </div>
                </CardContent>
              </Card>
            </div>
          </Section>

          {/* 图表色 */}
          <Section title="图表色板" desc="数据可视化的 5 色配色方案">
            <Card>
              <CardContent className="pt-6">
                <div className="flex gap-3">
                  <div className="flex-1 flex flex-col items-center gap-2">
                    <div className="h-20 w-full rounded-xl" style={{ backgroundColor: 'var(--chart-1)' }} />
                    <span className="text-xs font-mono text-muted-foreground">{"chart-1"}</span>
                  </div>
                  <div className="flex-1 flex flex-col items-center gap-2">
                    <div className="h-20 w-full rounded-xl" style={{ backgroundColor: 'var(--chart-2)' }} />
                    <span className="text-xs font-mono text-muted-foreground">{"chart-2"}</span>
                  </div>
                  <div className="flex-1 flex flex-col items-center gap-2">
                    <div className="h-20 w-full rounded-xl" style={{ backgroundColor: 'var(--chart-3)' }} />
                    <span className="text-xs font-mono text-muted-foreground">{"chart-3"}</span>
                  </div>
                  <div className="flex-1 flex flex-col items-center gap-2">
                    <div className="h-20 w-full rounded-xl" style={{ backgroundColor: 'var(--chart-4)' }} />
                    <span className="text-xs font-mono text-muted-foreground">{"chart-4"}</span>
                  </div>
                  <div className="flex-1 flex flex-col items-center gap-2">
                    <div className="h-20 w-full rounded-xl" style={{ backgroundColor: 'var(--chart-5)' }} />
                    <span className="text-xs font-mono text-muted-foreground">{"chart-5"}</span>
                  </div>
                </div>
              </CardContent>
            </Card>
          </Section>

          <Separator />

          {/* ═══════════════════════════════════════════════
              2. Button 按钮
              ═══════════════════════════════════════════════ */}
          <Section title="Button 按钮" desc="所有变体与尺寸">
            <Card>
              <CardContent className="pt-6 space-y-5">
                <div className="flex flex-wrap gap-3">
                  <Button><Send className="h-4 w-4 mr-2" />{"发送消息"}</Button>
                  <Button variant="secondary"><MessageSquare className="h-4 w-4 mr-2" />{"新对话"}</Button>
                  <Button variant="outline"><Code className="h-4 w-4 mr-2" />{"查看代码"}</Button>
                  <Button variant="ghost">{"取消"}</Button>
                  <Button variant="destructive"><Trash2 className="h-4 w-4 mr-2" />{"删除"}</Button>
                  <Button variant="link">{"链接按钮"}</Button>
                </div>
                <Separator />
                <div className="flex flex-wrap items-center gap-3">
                  <Button size="lg">{"大按钮"}</Button>
                  <Button size="default">{"默认按钮"}</Button>
                  <Button size="sm">{"小按钮"}</Button>
                  <Button size="icon"><Plus className="h-4 w-4" /></Button>
                </div>
                <Separator />
                <div className="flex flex-wrap gap-3">
                  <Button disabled>{"禁用状态"}</Button>
                  <Button variant="outline" disabled>{"禁用轮廓"}</Button>
                  <Button variant="secondary" disabled>{"禁用次要"}</Button>
                </div>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              3. Badge 徽章
              ═══════════════════════════════════════════════ */}
          <Section title="Badge 徽章" desc="标签与状态标识">
            <Card>
              <CardContent className="pt-6">
                <div className="flex flex-wrap gap-3">
                  <Badge>{"默认"}</Badge>
                  <Badge variant="secondary">{"次要"}</Badge>
                  <Badge variant="outline">{"轮廓"}</Badge>
                  <Badge variant="destructive">{"危险"}</Badge>
                  <Badge className="bg-success text-success-foreground">{"成功"}</Badge>
                  <Badge className="bg-ai-tool text-ai-tool-foreground"><Wrench className="h-3 w-3 mr-1" />{"工具调用"}</Badge>
                  <Badge className="bg-primary/20 text-primary"><Zap className="h-3 w-3 mr-1" />{"AI 生成"}</Badge>
                </div>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              4. Input / Textarea / Select 输入
              ═══════════════════════════════════════════════ */}
          <Section title="Input / Textarea / Select" desc="表单输入组件">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <Card>
                <CardHeader className="pb-3">
                  <CardTitle className="text-sm">{"文本输入"}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="space-y-2">
                    <Label htmlFor="name">{"用户名"}</Label>
                    <Input id="name" placeholder="请输入用户名..." />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="search">{"搜索"}</Label>
                    <div className="relative">
                      <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                      <Input id="search" className="pl-9" placeholder="搜索对话..." />
                    </div>
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="disabled">{"禁用"}</Label>
                    <Input id="disabled" placeholder="不可编辑" disabled />
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardHeader className="pb-3">
                  <CardTitle className="text-sm">{"多行 / 选择"}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="space-y-2">
                    <Label htmlFor="msg">{"消息"}</Label>
                    <Textarea id="msg" placeholder="输入你的提示词..." rows={3} />
                  </div>
                  <div className="space-y-2">
                    <Label>{"模型选择"}</Label>
                    <Select>
                      <SelectTrigger>
                        <SelectValue placeholder="选择模型" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="gpt4">GPT-4o</SelectItem>
                        <SelectItem value="claude">Claude 3.5</SelectItem>
                        <SelectItem value="gemini">Gemini Pro</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </CardContent>
              </Card>
            </div>
          </Section>

          {/* ═══════════════════════════════════════════════
              5. Checkbox / Radio / Switch 选择
              ═══════════════════════════════════════════════ */}
          <Section title="Checkbox / Radio / Switch" desc="选择控件">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              <Card>
                <CardHeader className="pb-3"><CardTitle className="text-sm">{"Checkbox"}</CardTitle></CardHeader>
                <CardContent className="space-y-3">
                  <div className="flex items-center gap-2">
                    <Checkbox id="c1" defaultChecked /><Label htmlFor="c1">{"启用流式输出"}</Label>
                  </div>
                  <div className="flex items-center gap-2">
                    <Checkbox id="c2" /><Label htmlFor="c2">{"保存对话历史"}</Label>
                  </div>
                  <div className="flex items-center gap-2">
                    <Checkbox id="c3" disabled /><Label htmlFor="c3" className="text-muted-foreground">{"禁用选项"}</Label>
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardHeader className="pb-3"><CardTitle className="text-sm">{"Radio Group"}</CardTitle></CardHeader>
                <CardContent>
                  <RadioGroup defaultValue="balanced">
                    <div className="flex items-center gap-2"><RadioGroupItem value="creative" id="r1" /><Label htmlFor="r1">{"创造力优先"}</Label></div>
                    <div className="flex items-center gap-2"><RadioGroupItem value="balanced" id="r2" /><Label htmlFor="r2">{"平衡模式"}</Label></div>
                    <div className="flex items-center gap-2"><RadioGroupItem value="precise" id="r3" /><Label htmlFor="r3">{"精确模式"}</Label></div>
                  </RadioGroup>
                </CardContent>
              </Card>
              <Card>
                <CardHeader className="pb-3"><CardTitle className="text-sm">{"Switch"}</CardTitle></CardHeader>
                <CardContent className="space-y-4">
                  <div className="flex items-center justify-between">
                    <Label htmlFor="s1">{"暗色模式"}</Label>
                    <Switch id="s1" />
                  </div>
                  <div className="flex items-center justify-between">
                    <Label htmlFor="s2">{"通知"}</Label>
                    <Switch id="s2" defaultChecked />
                  </div>
                  <div className="flex items-center justify-between">
                    <Label htmlFor="s3" className="text-muted-foreground">{"禁用"}</Label>
                    <Switch id="s3" disabled />
                  </div>
                </CardContent>
              </Card>
            </div>
          </Section>

          {/* ═══════════════════════════════════════════════
              6. Slider / Progress
              ═══════════════════════════════════════════════ */}
          <Section title="Slider / Progress" desc="范围与进度指示">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <Card>
                <CardHeader className="pb-3"><CardTitle className="text-sm">{"Slider"}</CardTitle></CardHeader>
                <CardContent className="space-y-6">
                  <div className="space-y-2">
                    <div className="flex justify-between"><Label>{"Temperature"}</Label><span className="text-sm text-muted-foreground">{sliderVal[0] / 100}</span></div>
                    <Slider value={sliderVal} onValueChange={setSliderVal} max={100} step={1} />
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardHeader className="pb-3"><CardTitle className="text-sm">{"Progress"}</CardTitle></CardHeader>
                <CardContent className="space-y-4">
                  <div className="space-y-2">
                    <div className="flex justify-between"><Label>{"生成进度"}</Label><span className="text-sm text-muted-foreground">{progress}%</span></div>
                    <Progress value={progress} />
                  </div>
                  <div className="space-y-2">
                    <Label>{"静态 50%"}</Label>
                    <Progress value={50} />
                  </div>
                </CardContent>
              </Card>
            </div>
          </Section>

          {/* ═══════════════════════════════════════════════
              7. Card 卡片
              ═══════════════════════════════════════════════ */}
          <Section title="Card 卡片" desc="信息展示容器">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2"><Bot className="h-5 w-5 text-primary" />{"AI 对话"}</CardTitle>
                  <CardDescription>{"与智能助手开始新对话"}</CardDescription>
                </CardHeader>
                <CardContent><p className="text-sm text-muted-foreground">{"支持多轮对话、上下文理解、代码生成等能力"}</p></CardContent>
                <CardFooter><Button className="w-full"><MessageSquare className="h-4 w-4 mr-2" />{"开始对话"}</Button></CardFooter>
              </Card>
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2"><Wrench className="h-5 w-5 text-ai-tool" />{"工具箱"}</CardTitle>
                  <CardDescription>{"AI 辅助工具合集"}</CardDescription>
                </CardHeader>
                <CardContent><p className="text-sm text-muted-foreground">{"代码审查、文本翻译、数据分析、图片理解"}</p></CardContent>
                <CardFooter><Button variant="outline" className="w-full"><Zap className="h-4 w-4 mr-2" />{"查看工具"}</Button></CardFooter>
              </Card>
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2"><Settings className="h-5 w-5 text-muted-foreground" />{"设置"}</CardTitle>
                  <CardDescription>{"自定义你的体验"}</CardDescription>
                </CardHeader>
                <CardContent><p className="text-sm text-muted-foreground">{"模型选择、系统提示词、外观主题"}</p></CardContent>
                <CardFooter><Button variant="secondary" className="w-full"><Settings className="h-4 w-4 mr-2" />{"打开设置"}</Button></CardFooter>
              </Card>
            </div>
          </Section>

          {/* ═══════════════════════════════════════════════
              8. Tabs 选项卡
              ═══════════════════════════════════════════════ */}
          <Section title="Tabs 选项卡" desc="内容分组切换">
            <Card>
              <CardContent className="pt-6">
                <Tabs defaultValue="chat">
                  <TabsList>
                    <TabsTrigger value="chat"><MessageSquare className="h-4 w-4 mr-1.5" />{"对话"}</TabsTrigger>
                    <TabsTrigger value="tools"><Wrench className="h-4 w-4 mr-1.5" />{"工具"}</TabsTrigger>
                    <TabsTrigger value="settings"><Settings className="h-4 w-4 mr-1.5" />{"设置"}</TabsTrigger>
                  </TabsList>
                  <TabsContent value="chat" className="mt-4">
                    <p className="text-sm text-muted-foreground">{"这里展示对话记录和消息界面。"}</p>
                  </TabsContent>
                  <TabsContent value="tools" className="mt-4">
                    <p className="text-sm text-muted-foreground">{"这里展示 AI 工具箱的内容。"}</p>
                  </TabsContent>
                  <TabsContent value="settings" className="mt-4">
                    <p className="text-sm text-muted-foreground">{"这里展示用户设置选项。"}</p>
                  </TabsContent>
                </Tabs>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              9. Alert 提示
              ═══════════════════════════════════════════════ */}
          <Section title="Alert 提示" desc="系统消息与通知">
            <div className="space-y-4">
              <Alert>
                <Info className="h-4 w-4" />
                <AlertTitle>{"提示"}</AlertTitle>
                <AlertDescription>{"AI 模型已更新到最新版本，支持更多工具调用。"}</AlertDescription>
              </Alert>
              <Alert variant="destructive">
                <AlertCircle className="h-4 w-4" />
                <AlertTitle>{"错误"}</AlertTitle>
                <AlertDescription>{"API 请求失败，请检查你的网络连接后重试。"}</AlertDescription>
              </Alert>
            </div>
          </Section>

          {/* ═══════════════════════════════════════════════
              10. Avatar 头像
              ═══════════════════════════════════════════════ */}
          <Section title="Avatar 头像" desc="用户与 AI 身份标识">
            <Card>
              <CardContent className="pt-6">
                <div className="flex items-center gap-4">
                  <Avatar className="h-12 w-12">
                    <AvatarImage src="https://api.dicebear.com/9.x/bottts-neutral/svg?seed=ai" alt="AI" />
                    <AvatarFallback className="bg-primary text-primary-foreground">AI</AvatarFallback>
                  </Avatar>
                  <Avatar className="h-12 w-12">
                    <AvatarImage src="https://api.dicebear.com/9.x/notionists/svg?seed=user" alt="User" />
                    <AvatarFallback className="bg-secondary text-secondary-foreground">ME</AvatarFallback>
                  </Avatar>
                  <Avatar className="h-12 w-12">
                    <AvatarFallback className="bg-accent text-accent-foreground">OP</AvatarFallback>
                  </Avatar>
                  <Avatar className="h-10 w-10">
                    <AvatarFallback className="bg-muted text-muted-foreground text-xs">{"小"}</AvatarFallback>
                  </Avatar>
                  <Avatar className="h-8 w-8">
                    <AvatarFallback className="bg-primary/20 text-primary text-[10px]">XS</AvatarFallback>
                  </Avatar>
                </div>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              11. Accordion 手风琴
              ═══════════════════════════════════════════════ */}
          <Section title="Accordion 手风琴" desc="可折叠内容面板">
            <Card>
              <CardContent className="pt-6">
                <Accordion type="single" collapsible className="w-full">
                  <AccordionItem value="i1">
                    <AccordionTrigger>{"这个 AI 支持哪些功能？"}</AccordionTrigger>
                    <AccordionContent>{"支持多轮对话、代码生成与审查、文本翻译、数据分析、图片理解、工具调用等。"}</AccordionContent>
                  </AccordionItem>
                  <AccordionItem value="i2">
                    <AccordionTrigger>{"对话历史会保存多久？"}</AccordionTrigger>
                    <AccordionContent>{"对话历史默认保存 30 天，你也可以在设置中选择永久保存或手动清除。"}</AccordionContent>
                  </AccordionItem>
                  <AccordionItem value="i3">
                    <AccordionTrigger>{"如何切换 AI 模型？"}</AccordionTrigger>
                    <AccordionContent>{"在设置页面或对话输入框左侧的模型选择器中即可快速切换。"}</AccordionContent>
                  </AccordionItem>
                </Accordion>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              12. Table 表格
              ═══════════════════════════════════════════════ */}
          <Section title="Table 表格" desc="结构化数据展示">
            <Card>
              <CardContent className="pt-6">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>{"模型"}</TableHead>
                      <TableHead>{"供应商"}</TableHead>
                      <TableHead>{"延迟"}</TableHead>
                      <TableHead className="text-right">{"状态"}</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    <TableRow>
                      <TableCell className="font-medium">GPT-4o</TableCell>
                      <TableCell>OpenAI</TableCell>
                      <TableCell>{"~320ms"}</TableCell>
                      <TableCell className="text-right"><Badge className="bg-success/15 text-success border-0">{"在线"}</Badge></TableCell>
                    </TableRow>
                    <TableRow>
                      <TableCell className="font-medium">Claude 3.5</TableCell>
                      <TableCell>Anthropic</TableCell>
                      <TableCell>{"~280ms"}</TableCell>
                      <TableCell className="text-right"><Badge className="bg-success/15 text-success border-0">{"在线"}</Badge></TableCell>
                    </TableRow>
                    <TableRow>
                      <TableCell className="font-medium">Gemini Pro</TableCell>
                      <TableCell>Google</TableCell>
                      <TableCell>{"~450ms"}</TableCell>
                      <TableCell className="text-right"><Badge className="bg-ai-tool/15 text-ai-tool border-0">{"维护中"}</Badge></TableCell>
                    </TableRow>
                    <TableRow>
                      <TableCell className="font-medium">Llama 3</TableCell>
                      <TableCell>Meta</TableCell>
                      <TableCell>{"-"}</TableCell>
                      <TableCell className="text-right"><Badge variant="outline" className="text-muted-foreground">{"离线"}</Badge></TableCell>
                    </TableRow>
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              13. Toggle / ToggleGroup
              ═══════════════════════════════════════════════ */}
          <Section title="Toggle / ToggleGroup" desc="切换按钮与分组">
            <Card>
              <CardContent className="pt-6 space-y-4">
                <div className="flex flex-wrap gap-2">
                  <Toggle aria-label="Bold"><Bold className="h-4 w-4" /></Toggle>
                  <Toggle aria-label="Italic"><Italic className="h-4 w-4" /></Toggle>
                  <Toggle aria-label="Underline"><Underline className="h-4 w-4" /></Toggle>
                </div>
                <Separator />
                <ToggleGroup type="single" defaultValue="left">
                  <ToggleGroupItem value="left" aria-label="左对齐"><AlignLeft className="h-4 w-4" /></ToggleGroupItem>
                  <ToggleGroupItem value="center" aria-label="居中"><AlignCenter className="h-4 w-4" /></ToggleGroupItem>
                  <ToggleGroupItem value="right" aria-label="右对齐"><AlignRight className="h-4 w-4" /></ToggleGroupItem>
                </ToggleGroup>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              14. Dialog / Sheet / Popover / Dropdown / Tooltip
              ═══════════════════════════════════════════════ */}
          <Section title="Dialog / Sheet / Popover / Dropdown / Tooltip" desc="弹出层与浮层组件">
            <Card>
              <CardContent className="pt-6">
                <div className="flex flex-wrap gap-3">
                  {/* Dialog */}
                  <Dialog>
                    <DialogTrigger asChild><Button variant="outline"><Eye className="h-4 w-4 mr-2" />{"Dialog"}</Button></DialogTrigger>
                    <DialogContent>
                      <DialogHeader>
                        <DialogTitle>{"确认操作"}</DialogTitle>
                        <DialogDescription>{"你确定要清除所有对话记录吗？此操作不可撤销。"}</DialogDescription>
                      </DialogHeader>
                      <DialogFooter>
                        <Button variant="secondary">{"取消"}</Button>
                        <Button variant="destructive">{"确认清除"}</Button>
                      </DialogFooter>
                    </DialogContent>
                  </Dialog>

                  {/* Sheet */}
                  <Sheet>
                    <SheetTrigger asChild><Button variant="outline"><Settings className="h-4 w-4 mr-2" />{"Sheet"}</Button></SheetTrigger>
                    <SheetContent>
                      <SheetHeader>
                        <SheetTitle>{"设置面板"}</SheetTitle>
                        <SheetDescription>{"调整你的 AI 助手偏好设置"}</SheetDescription>
                      </SheetHeader>
                      <div className="mt-6 space-y-4">
                        <div className="flex items-center justify-between">
                          <Label>{"暗色模式"}</Label><Switch />
                        </div>
                        <div className="flex items-center justify-between">
                          <Label>{"流式输出"}</Label><Switch defaultChecked />
                        </div>
                      </div>
                    </SheetContent>
                  </Sheet>

                  {/* Popover */}
                  <Popover>
                    <PopoverTrigger asChild><Button variant="outline"><Info className="h-4 w-4 mr-2" />{"Popover"}</Button></PopoverTrigger>
                    <PopoverContent className="w-72">
                      <div className="space-y-2">
                        <h4 className="font-medium text-sm text-popover-foreground">{"模型信息"}</h4>
                        <p className="text-xs text-muted-foreground">{"当前使用 GPT-4o，上下文窗口 128K tokens，支持视觉和工具调用。"}</p>
                      </div>
                    </PopoverContent>
                  </Popover>

                  {/* Dropdown */}
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild><Button variant="outline"><MoreHorizontal className="h-4 w-4 mr-2" />{"Dropdown"}</Button></DropdownMenuTrigger>
                    <DropdownMenuContent>
                      <DropdownMenuLabel>{"操作"}</DropdownMenuLabel>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem><Copy className="h-4 w-4 mr-2" />{"复制"}</DropdownMenuItem>
                      <DropdownMenuItem><Share2 className="h-4 w-4 mr-2" />{"分享"}</DropdownMenuItem>
                      <DropdownMenuItem><Download className="h-4 w-4 mr-2" />{"导出"}</DropdownMenuItem>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem className="text-destructive"><Trash2 className="h-4 w-4 mr-2" />{"删除"}</DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>

                  {/* Tooltip */}
                  <Tooltip>
                    <TooltipTrigger asChild><Button variant="outline" size="icon"><Bell className="h-4 w-4" /></Button></TooltipTrigger>
                    <TooltipContent><p>{"通知中心"}</p></TooltipContent>
                  </Tooltip>
                </div>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              15. Breadcrumb / Pagination 导航
              ═══════════════════════════════════════════════ */}
          <Section title="Breadcrumb / Pagination" desc="导航与分页">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <Card>
                <CardHeader className="pb-3"><CardTitle className="text-sm">{"Breadcrumb"}</CardTitle></CardHeader>
                <CardContent>
                  <Breadcrumb>
                    <BreadcrumbList>
                      <BreadcrumbItem><BreadcrumbLink href="#">{"首页"}</BreadcrumbLink></BreadcrumbItem>
                      <BreadcrumbSeparator />
                      <BreadcrumbItem><BreadcrumbLink href="#">{"对话"}</BreadcrumbLink></BreadcrumbItem>
                      <BreadcrumbSeparator />
                      <BreadcrumbItem><BreadcrumbPage>{"React 组件开发"}</BreadcrumbPage></BreadcrumbItem>
                    </BreadcrumbList>
                  </Breadcrumb>
                </CardContent>
              </Card>
              <Card>
                <CardHeader className="pb-3"><CardTitle className="text-sm">{"Pagination"}</CardTitle></CardHeader>
                <CardContent>
                  <Pagination>
                    <PaginationContent>
                      <PaginationItem><PaginationPrevious href="#" /></PaginationItem>
                      <PaginationItem><PaginationLink href="#">1</PaginationLink></PaginationItem>
                      <PaginationItem><PaginationLink href="#" isActive>2</PaginationLink></PaginationItem>
                      <PaginationItem><PaginationLink href="#">3</PaginationLink></PaginationItem>
                      <PaginationItem><PaginationEllipsis /></PaginationItem>
                      <PaginationItem><PaginationNext href="#" /></PaginationItem>
                    </PaginationContent>
                  </Pagination>
                </CardContent>
              </Card>
            </div>
          </Section>

          {/* ═══════════════════════════════════════════════
              16. Skeleton 骨架屏
              ═══════════════════════════════════════════════ */}
          <Section title="Skeleton 骨架屏" desc="加载占位">
            <Card>
              <CardContent className="pt-6">
                <div className="flex items-start gap-3">
                  <Skeleton className="h-10 w-10 rounded-full" />
                  <div className="flex-1 space-y-2">
                    <Skeleton className="h-4 w-28" />
                    <Skeleton className="h-4 w-full" />
                    <Skeleton className="h-4 w-3/4" />
                  </div>
                </div>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              17. ScrollArea
              ═══════════════════════════════════════════════ */}
          <Section title="ScrollArea 滚动区域" desc="自定义滚动容器">
            <Card>
              <CardContent className="pt-6">
                <ScrollArea className="h-40 rounded-lg border border-border p-4">
                  {Array.from({ length: 20 }).map((_, i) => (
                    <div key={i} className="py-2 border-b border-border/50 last:border-0 text-sm text-foreground">
                      {`对话 ${i + 1}: 这是一条模拟的对话记录`}
                    </div>
                  ))}
                </ScrollArea>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              18. Typography 排版
              ═══════════════════════════════════════════════ */}
          <Section title="Typography 排版" desc="字体与文本层级">
            <Card>
              <CardContent className="pt-6 space-y-3">
                <h1 className="text-4xl font-bold text-foreground">{"Heading 1 - 标题一"}</h1>
                <h2 className="text-3xl font-semibold text-foreground">{"Heading 2 - 标题二"}</h2>
                <h3 className="text-2xl font-semibold text-foreground">{"Heading 3 - 标题三"}</h3>
                <h4 className="text-xl font-medium text-foreground">{"Heading 4 - 标题四"}</h4>
                <p className="text-base leading-relaxed text-foreground">{"正文文字 Body — 这是用于长段落阅读的正文样式，行高适中，确保阅读舒适性。"}</p>
                <p className="text-sm text-muted-foreground">{"辅助文字 Caption — 用于描述、注释等次要信息。"}</p>
                <p className="text-xs font-mono text-muted-foreground">{"等宽字体 Mono — const greeting = \"Hello AI\""}</p>
              </CardContent>
            </Card>
          </Section>

          {/* ═══════════════════════════════════════════════
              19. AI 对话完整预览
              ═══════════════════════════════════════════════ */}
          <Section title="AI 对话预览" desc="模拟完整的聊天与工具调用场景">
            <Card className="overflow-hidden">
              <CardContent className="p-6 space-y-5 bg-background">
                <ChatBubbleAI />
                <ChatBubbleUser />
                <ToolCallCard />
              </CardContent>
              <Separator />
              <div className="p-4 bg-card">
                <div className="flex gap-2">
                  <Input placeholder="输入你的问题..." className="bg-background" />
                  <Button size="icon"><Send className="h-4 w-4" /></Button>
                </div>
              </div>
            </Card>
          </Section>

          <div className="h-8" />
        </main>
      </div>
    </TooltipProvider>
  )
}
