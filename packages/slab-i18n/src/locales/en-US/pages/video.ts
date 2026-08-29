export const video = {
  modelPicker: {
    groupLabel: 'Video Models',
    optionDownloadInHub: '{{model}} (Download in Hub)',
  },
  options: {
    sampleMethods: {
      auto: 'Auto',
      euler: 'Euler',
      euler_a: 'Euler A',
      lcm: 'LCM',
      'dpm++2m': 'DPM++ 2M',
    },
    schedulers: {
      auto: 'Auto',
      discrete: 'Discrete',
      karras: 'Karras',
    },
  },
  workbench: {
    configTitle: 'Configuration',
    prompt: {
      label: 'Creative Prompt',
      placeholder: 'Describe the scene in cinematic detail...',
    },
    negativePrompt: {
      placeholder: 'Blurry, low quality, distorted...',
    },
    fields: {
      frames: 'Frames',
      fps: 'FPS',
      referenceImage: 'Reference Image',
      advanced: 'Advanced Parameters',
    },
    referenceImage: {
      readyTitle: 'Reference frame ready',
      readyDescription: 'Slab will use this image as the starting frame.',
      uploadTitle: 'Drop PNG/JPG or click to upload',
      uploadDescription: 'Optional starting frame for motion generation.',
      previewAlt: 'Reference image preview',
      removeAria: 'Remove reference image',
    },
    actions: {
      generate: 'Generate Video',
      cancel: 'Cancel current render',
    },
    stage: {
      toggleScale: 'Toggle stage scale',
      downloadVideo: 'Download video',
      renderStatus: 'Render Status',
      clipSpec: 'Clip Spec',
      canvas: 'Canvas',
      clipSpecValue: '{{frames}} frames - {{fps}} fps',
    },
  },
  history: {
    loading: 'Loading recent video generations...',
    description: 'Reopen generated clips, prompts, and render settings from recent video tasks.',
    empty: 'No video history yet. Completed renders will show up here.',
    detailTitle: 'Video Task Detail',
    noArtifact: 'This task has not produced a persisted video artifact yet.',
    compareArtifact: 'Comparison artifact {{index}}',
    actions: {
      compare: 'Compare',
      openWorkspace: 'Open in Workspace',
      removeCompare: 'Remove compare',
    },
    fields: {
      clip: 'Clip',
    },
  },
  progress: {
    title: 'Render progress',
    running: 'Runtime is rendering frames',
    finalizing: 'Finalizing video artifact',
  },
  stage: {
    title: {
      ready: 'Render Ready',
      rendering: 'Rendering Preview',
      idle: 'Preview Canvas',
    },
    description: {
      ready: 'Your generated clip is ready to review, resize in stage, or download locally.',
      rendering: 'Generating {{frames}} frames at {{fps}} fps. Slab is polling the runtime for completion.',
      idle: 'Generated video will appear here after processing. Ready for cinematic render.',
    },
    status: {
      ready: 'Render complete',
      rendering: 'Generating',
      queued: 'Ready to render',
      awaitingPrompt: 'Awaiting prompt',
    },
    footerHint: {
      ready: 'Generated clip is saved locally and available for download.',
      polling: 'Polling every {{seconds}} seconds until the runtime finishes.',
      estimate: 'Estimated clip length: {{seconds}} seconds.',
      downloadFirst: 'Download a local diffusion model in Hub before starting a render.',
    },
  },
  toast: {
    started: 'Video generation started ({{frames}} frames at {{fps}} fps)...',
    generated: 'Video generated!',
    historyDetailFailed: 'Failed to open video history detail: {{message}}',
    completedWithoutPath: 'Video generation completed without a video path',
    resultFetchFailed: 'Failed to fetch video result: {{message}}',
  },
  error: {
    selectDownloadedModel: 'Selected model is not downloaded. Please download it first in Hub.',
    chooseImageFile: 'Please choose an image file',
    generationFailed: 'Video generation failed',
  },
} as const;
