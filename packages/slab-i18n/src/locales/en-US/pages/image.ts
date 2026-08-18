export const image = {
  modelPicker: {
    groupLabel: 'Image Models',
  },
  options: {
    mode: {
      txt2img: 'Text to Image',
      img2img: 'Image to Image',
    },
    sampleMethods: {
      auto: 'Auto',
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
      auto: 'Auto',
      discrete: 'Discrete',
      karras: 'Karras',
      exponential: 'Exponential',
      ays: 'AYS',
      gits: 'GITS',
    },
  },
  workbench: {
    sectionTitle: 'Generation Parameters',
    initImage: {
      currentLabel: 'Init Image',
      uploadLabel: 'Upload Init Image',
      chooseTitle: 'Click to choose an image',
      chooseDescription: 'PNG/JPEG for img2img mode',
      previewAlt: 'Init image preview',
      removeAria: 'Remove init image',
    },
    prompt: {
      label: 'Prompt',
      placeholder: 'A cinematic portrait with moody rim light...',
    },
    negativePrompt: {
      placeholder: 'blurry, low quality, distorted...',
    },
    dimensions: {
      label: 'Dimensions',
    },
    imageCount: {
      label: 'Number of Images',
      option_one: '{{count}} image',
      option_other: '{{count}} images',
    },
    advanced: {
      title: 'Advanced Settings',
      clipSkip: 'CLIP Skip',
      eta: 'Eta (DDIM)',
    },
    actions: {
      loadingPreset: 'Loading preset...',
      preparingModel: 'Preparing model...',
      generate: 'Generate Images',
      cancel: 'Cancel generation',
    },
    emptyState: {
      generatingTitle: 'Generating images...',
      readyTitle: 'Ready to create?',
      generatingDescription: 'Your task is running. Generated images will appear here automatically.',
      readyDescription:
        'Enter a prompt and adjust the parameters to see your imagination come to life.',
      taskRunning: 'Task running',
      viewHistory: 'View History',
      getInspired: 'Get Inspired',
    },
    gallery: {
      title: 'Generated Images',
      description: 'Review the latest renders, zoom in for detail, or download the best take.',
      count_one: '{{count}} image',
      count_other: '{{count}} images',
      zoomAria: 'Zoom image',
      downloadAria: 'Download image',
      previewAlt: 'Generated image preview',
    },
  },
  history: {
    loading: 'Loading recent image generations...',
    description: 'Reopen prompts, previews, and saved outputs from recent image tasks.',
    empty: 'No image history yet. Your completed image tasks will appear here.',
    detailTitle: 'Image Task Detail',
    fields: {
      mode: 'Mode',
    },
  },
  progress: {
    title: 'Generation progress',
    running: 'Runtime is producing samples',
    finalizing: 'Finalizing generated assets',
  },
  toast: {
    generated_one: 'Generated {{count}} image!',
    generated_other: 'Generated {{count}} images!',
    historyDetailFailed: 'Failed to open image history detail: {{message}}',
    resultFetchFailed: 'Failed to fetch generation result: {{message}}',
  },
  error: {
    modelPresetLoading: 'Model preset is still loading',
    uploadInitImage: 'Please upload an init image for img2img mode',
    selectModelFirst: 'Please select an image model first.',
    selectedModelUnavailable: 'Selected model is not available',
    generationFailed: 'Image generation failed',
  },
} as const;
