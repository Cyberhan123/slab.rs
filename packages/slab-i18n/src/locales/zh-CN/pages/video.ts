export const video = {
  modelPicker: {
    groupLabel: '视频模型',
    optionDownloadInHub: '{{model}}（请先在模型库下载）',
  },
  options: {
    sampleMethods: {
      auto: '自动',
      euler: 'Euler',
      euler_a: 'Euler A',
      lcm: 'LCM',
      'dpm++2m': 'DPM++ 2M',
    },
    schedulers: {
      auto: '自动',
      discrete: '离散',
      karras: 'Karras',
    },
  },
  workbench: {
    configTitle: '配置',
    prompt: {
      label: '创意提示词',
      placeholder: '用电影镜头语言详细描述这个场景...',
    },
    negativePrompt: {
      placeholder: '模糊、低质量、畸变...',
    },
    fields: {
      frames: '帧数',
      fps: '帧率',
      referenceImage: '参考图像',
      advanced: '高级参数',
    },
    referenceImage: {
      readyTitle: '参考帧已就绪',
      readyDescription: 'Slab 会将这张图像作为起始帧使用。',
      uploadTitle: '拖放 PNG/JPG，或点击上传',
      uploadDescription: '可选的运动生成起始帧。',
      previewAlt: '参考图像预览',
      removeAria: '移除参考图像',
    },
    actions: {
      generate: '生成视频',
      cancel: '取消当前渲染',
    },
    stage: {
      toggleScale: '切换画布缩放',
      downloadVideo: '下载视频',
      renderStatus: '渲染状态',
      clipSpec: '片段规格',
      canvas: '画布',
      clipSpecValue: '{{frames}} 帧 - {{fps}} fps',
    },
  },
  history: {
    loading: '正在加载最近的视频生成记录...',
    description: '重新查看最近视频任务的生成片段、提示词和渲染参数。',
    empty: '暂无视频历史。已完成的渲染结果会显示在这里。',
    detailTitle: '视频任务详情',
    noArtifact: '这个任务暂时还没有可持久化查看的视频产物。',
    compareArtifact: '对比产物 {{index}}',
    actions: {
      compare: '加入对比',
      openWorkspace: '在工作区打开',
      removeCompare: '移出对比',
    },
    fields: {
      clip: '片段',
    },
  },
  progress: {
    title: '渲染进度',
    running: '运行时正在渲染帧',
    finalizing: '正在整理视频产物',
  },
  stage: {
    title: {
      ready: '渲染已完成',
      rendering: '渲染预览中',
      idle: '预览画布',
    },
    description: {
      ready: '生成的视频已经可以查看、在画布中调整展示方式，或下载到本地。',
      rendering: '正在生成 {{frames}} 帧、{{fps}} fps 的视频。Slab 会持续轮询运行时状态。',
      idle: '处理完成后，生成的视频会显示在这里，随时可以开始电影感渲染。',
    },
    status: {
      ready: '渲染完成',
      rendering: '生成中',
      queued: '可开始渲染',
      awaitingPrompt: '等待提示词',
    },
    footerHint: {
      ready: '生成的视频已保存在本地，可随时下载。',
      polling: '每 {{seconds}} 秒轮询一次，直到运行时处理完成。',
      estimate: '预计片段时长：{{seconds}} 秒。',
      downloadFirst: '开始渲染前，请先在模型库中下载本地扩散模型。',
    },
  },
  toast: {
    started: '视频生成已开始（{{frames}} 帧，{{fps}} fps）...',
    generated: '视频已生成！',
    historyDetailFailed: '打开视频历史详情失败：{{message}}',
    completedWithoutPath: '视频生成完成，但没有返回视频路径',
    resultFetchFailed: '获取视频结果失败：{{message}}',
  },
  error: {
    selectDownloadedModel: '所选模型尚未下载，请先在模型库中下载。',
    chooseImageFile: '请选择图像文件',
    generationFailed: '视频生成失败',
  },
} as const;
