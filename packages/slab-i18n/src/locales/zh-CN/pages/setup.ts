export const setup = {
  header: {
    title: '初始化',
    subtitle: '准备本地运行时依赖',
  },
  checking: {
    title: '正在检查桌面环境',
    description: '正在检查本地 Slab host、打包运行时和 FFmpeg 可用性。',
    wait: '请稍等片刻。',
  },
  hostError: {
    title: '无法连接本地 host',
    hint: '确认 <code>slab-server</code> 正在运行，然后重新加载此页面。',
    reload: '重新加载',
  },
  errors: {
    failedBeforeFinish: '初始化未能完成。',
  },
  badge: {
    running: '运行中',
    failed: '失败',
    complete: '完成',
  },
  hero: {
    eyebrow: '桌面初始化',
    titleBundled: 'Slab 正在检查本地工具。',
    titlePackaged: 'Slab 正在准备本地运行时。',
    descriptionBundled:
      '此安装已在 <code>resources/libs</code> 中包含运行时载荷。Slab 正在检查 FFmpeg，并确认托管运行时 worker 可以启动。',
    descriptionMacosMissing:
      '此 macOS 构建需要在 <code>resources/libs</code> 中找到运行时库。Slab 正在启动本地 worker 前检查打包文件。',
    descriptionPackaged:
      'Slab 会复用当前版本的发行版 CAB 载荷，根据内嵌 manifest 校验后安装到 <code>resources/libs</code>，再检查 FFmpeg 并重启托管运行时 worker。',
  },
  metrics: {
    runtimePayload: {
      label: '运行时载荷',
      installed: '已本地安装',
      missingBundled: '正在检查打包文件',
      needsSetup: '需要初始化',
    },
    ffmpeg: {
      label: 'FFmpeg',
      available: '可用',
      willBeInstalled: '将自动安装',
    },
    backendWorkers: {
      label: '后端 Worker',
      ready: '{{ready}}/{{total}} 就绪',
      notReported: '未上报',
    },
  },
  currentStage: '当前阶段',
  progressHint: {
    activePackaged: '保持此窗口打开，直到本地初始化任务完成运行时配置。',
    activeBundled: '保持此窗口打开，Slab 正在检查 FFmpeg 并确认随包运行时已就绪。',
    succeeded: '初始化已完成。Slab 会自动进入应用。',
  },
  actions: {
    retry: '重试初始化',
    launching: '正在启动 Slab...',
    checkingPrerequisites: '正在检查桌面前置条件',
    provisioning: '正在初始化',
  },
  stages: {
    failed: '初始化失败',
    finished: '初始化完成',
    checkingPrerequisites: '正在检查桌面前置条件',
    starting: '正在启动初始化',
    verifyingInstalledRuntime: '正在验证已安装运行时',
    preparingRuntime: '正在准备 Slab 运行时',
    checkingEnvironment: '正在检查桌面环境',
    preparingEnvironment: '正在准备环境',
  },
  hints: {
    step: '第 {{step}} / {{count}} 步',
    failedBundled: '查看下方错误后，重试本地前置条件检查。',
    failedPackaged: '查看下方错误后，重试初始化任务。',
    succeededBundled: 'FFmpeg 和本地运行时检查已完成。即将启动 Slab。',
    succeededPackaged: '运行时载荷已就绪。即将启动 Slab。',
    startingBundled: '正在检查已安装运行时，并确认 FFmpeg 是否可用。',
    startingPackaged: '正在创建初始化任务并连接本地 host。',
    runningBundled: '正在检查 FFmpeg 运行时可用性，并确认本地 worker 已就绪。',
    runningPackaged: '正在下载载荷、校验 CAB、检查 FFmpeg 并重启运行时 worker。',
    idleBundled: '正在检查本地桌面安装和 FFmpeg 可用性。',
    idlePackaged: '正在检查本地桌面安装。',
  },
  summary: {
    failedBundled: '桌面前置条件检查在初始化完成前停止。',
    failedPackaged: '初始化在完成前停止。',
    complete: '100% 完成',
    percentComplete: '{{percentage}}% 完成',
    stage: '阶段 {{step}}/{{count}}',
    startingBundled: '正在检查已安装运行时...',
    startingPackaged: '正在创建初始化任务...',
    runningBundled: '正在检查 FFmpeg 和本地 worker...',
    runningPackaged: '正在等待进度更新...',
    idle: '等待开始',
  },
} as const;
