export const image = {
  modelPicker: {
    groupLabel: '图像模型',
  },
  options: {
    mode: {
      txt2img: '文生图',
      img2img: '图生图',
    },
    sampleMethods: {
      auto: '自动',
      euler: 'Euler',
      euler_a: 'Euler A',
      heun: 'Heun',
      dpm2: 'DPM2',
      'dpm++2s_a': 'DPM++ 2S a',
      'dpm++2m': 'DPM++ 2M',
      'dpm++2mv2': 'DPM++ 2M v2',
      lcm: 'LCM',
      ipndm: 'iPNDM',
      ipndm_v: 'iPNDM V',
    },
    schedulers: {
      auto: '自动',
      discrete: '离散',
      karras: 'Karras',
      exponential: '指数',
      ays: 'AYS',
      gits: 'GITS',
    },
  },
  workbench: {
    sectionTitle: '生成参数',
    initImage: {
      currentLabel: '初始图像',
      uploadLabel: '上传初始图像',
      chooseTitle: '点击选择图像',
      chooseDescription: '图生图模式支持 PNG/JPEG',
      previewAlt: '初始图像预览',
      removeAria: '移除初始图像',
    },
    prompt: {
      label: '提示词',
      placeholder: '一张带有电影感边缘光的人像...',
    },
    negativePrompt: {
      placeholder: '模糊、低质量、畸变...',
    },
    dimensions: {
      label: '尺寸',
    },
    imageCount: {
      label: '生成数量',
      option_one: '{{count}} 张图像',
      option_other: '{{count}} 张图像',
    },
    advanced: {
      title: '高级设置',
      clipSkip: 'CLIP Skip',
      eta: 'Eta (DDIM)',
    },
    actions: {
      loadingPreset: '正在加载预设...',
      preparingModel: '正在准备模型...',
      generate: '生成图像',
      cancel: '取消生成',
    },
    emptyState: {
      generatingTitle: '正在生成图像...',
      readyTitle: '准备开始创作？',
      generatingDescription: '任务正在运行，生成的图像会自动显示在这里。',
      readyDescription: '输入提示词并调整参数，让你的想象开始成像。',
      taskRunning: '任务运行中',
      viewHistory: '查看历史',
      getInspired: '获取灵感',
    },
    gallery: {
      title: '已生成图像',
      description: '查看最新结果，可放大细节，也可下载最满意的一张。',
      count_one: '{{count}} 张图像',
      count_other: '{{count}} 张图像',
      zoomAria: '放大图像',
      downloadAria: '下载图像',
      previewAlt: '生成图像预览',
    },
  },
  history: {
    loading: '正在加载最近的图像生成记录...',
    description: '重新查看最近图像任务的提示词、预览图和已保存输出。',
    empty: '暂无图像历史。已完成的图像任务会显示在这里。',
    detailTitle: '图像任务详情',
    fields: {
      mode: '模式',
    },
  },
  progress: {
    title: '生成进度',
    running: '运行时正在采样生成',
    finalizing: '正在整理生成产物',
  },
  toast: {
    generated_one: '已生成 {{count}} 张图像！',
    generated_other: '已生成 {{count}} 张图像！',
    historyDetailFailed: '打开图像历史详情失败：{{message}}',
    resultFetchFailed: '获取生成结果失败：{{message}}',
  },
  error: {
    modelPresetLoading: '模型预设仍在加载中',
    uploadInitImage: '图生图模式请先上传初始图像',
    selectModelFirst: '请先选择图像模型。',
    selectedModelUnavailable: '所选模型不可用',
    generationFailed: '图像生成失败',
  },
} as const;
