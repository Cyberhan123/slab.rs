export const hub = {
  header: {
    title: 'Hub',
    subtitle: 'Models Repository',
  },
  hero: {
    badge: 'New release',
    titleLead: 'Shape your local',
    titleAccent: 'model catalog.',
    description:
      'Import JSON manifests or .slab packs into the catalog, then download the local runtimes you want from each card when you are ready to use them.',
    importModel: 'Import model',
    refreshCatalog: 'Refresh catalog',
  },
  summary: {
    catalogEntries: 'Catalog entries',
    catalogDescription_one: '{{count}} backend currently mapped',
    catalogDescription_other: '{{count}} backends currently mapped',
    readyLocally: 'Ready locally',
    pendingDescription_one: '{{count}} download currently syncing',
    pendingDescription_other: '{{count}} downloads currently syncing',
    readyDescription:
      'Imported packs stay in the catalog until you pull a local runtime copy',
  },
  filters: {
    categories: {
      all: 'All models',
      language: 'Large language',
      vision: 'Vision',
      audio: 'Audio',
      coding: 'Coding',
      embedding: 'Embedding',
    },
    statuses: {
      all: 'All statuses',
      ready: 'Ready',
      downloading: 'Downloading',
      not_downloaded: 'Not downloaded',
      error: 'Error',
    },
    statusPlaceholder: 'Status',
  },
  alerts: {
    loadFailedTitle: 'Model catalog failed to load',
  },
  states: {
    loadingTitle: 'Loading model catalog',
    loadingDescription: 'Fetching model entries and runtime status.',
    emptyFilteredTitle: 'No model entries match the current filters',
    emptyFilteredDescription:
      'Try another category, adjust status, or import a new model manifest or pack.',
  },
  catalog: {
    emptyPageTitle: 'No cards on this page',
    emptyPageDescription: 'Try another page or relax the active filters.',
    runtime: 'Runtime',
    vad: 'VAD',
    download: 'Download',
    downloading: 'Downloading...',
    source: 'Source',
    updatedAt: 'Updated {{value}}',
    downloadRunning: 'Download task is running',
    downloadPendingDescription:
      'Fetching model files into local storage. The card will refresh when the runtime path is ready.',
    downloadIdleDescription:
      'Import only adds this pack to the catalog. Download it when you want a local runtime copy.',
    descriptions: {
      pending:
        'This {{backend}} entry is syncing into the local runtime catalog. Once the download finishes, the runtime path and readiness state will update automatically.',
      local:
        'Local {{backend}} model ready for inference. The manifest is already connected to a runtime path and can be used without leaving this workspace.',
      imported:
        'Imported {{backend}} manifest from {{repo}}. It is listed in the catalog now, and you can download the actual model files from this card when you need a local runtime copy.',
    },
    actions: {
      enhanceAria: 'Enhance {{model}} config',
      deleteAria: 'Delete {{model}}',
    },
    kind: {
      local: 'Local',
      cloud: 'Cloud',
    },
    backend: {
      llama: 'Llama',
      whisper: 'Whisper',
      diffusion: 'Diffusion',
    },
    unknownTime: 'Unknown',
    configuredRepository: 'the configured repository',
  },
  dialogs: {
    create: {
      title: 'Import model',
      description:
        'Upload a .slab model pack. Import only adds the entry to the catalog. Provider credentials stay in Settings, and supported local models can be downloaded later from their catalog cards.',
      modelPackLabel: 'Model pack',
      selectedDescription:
        'This pack will be validated, stored, and turned into a catalog entry without pulling remote model files yet.',
      emptyDescription: 'Choose a .slab pack to import a model entry.',
      submit: 'Import model',
    },
    delete: {
      title: 'Delete model entry?',
      descriptionWithModel:
        'Remove <strong>{{model}}</strong> from the model catalog and delete its stored .slab pack. This does not delete any downloaded model file on disk.',
      descriptionFallback: 'Remove this model entry from the catalog.',
      cancel: 'Cancel',
      confirm: 'Delete entry',
    },
  },
  sheet: {
    title: 'Model config document',
    description:
      'Pack declarations stay as the source of truth. You can only switch preset and variant here; backend fields remain locked and read-only.',
    loading: 'Loading model config document...',
    failedLoadTitle: 'Failed to load enhancement config',
    selectionWarningTitle: 'Selection warning',
    blocks: {
      displayName: 'Display name',
      backend: 'Backend',
      preset: 'Preset',
      variant: 'Variant',
      presetPlaceholder: 'Select a preset',
      variantPlaceholder: 'Select a variant',
      close: 'Close',
      saveSelection: 'Save selection',
      packLocked: 'Pack locked',
      notSet: 'Not set',
      enabled: 'Enabled',
      disabled: 'Disabled',
      origin: {
        pack_manifest: 'Pack manifest',
        selected_preset: 'Preset',
        selected_variant: 'Variant',
        selected_backend_config: 'Backend config',
        pmid_fallback: 'PMID fallback',
        derived: 'Derived',
      },
    },
  },
  toast: {
    imported: 'Model imported to catalog.',
    importFailed: 'Failed to import model.',
    downloaded: 'Model downloaded.',
    downloadFailed: 'Model download failed.',
    downloadStarted: 'Download started.',
    removed: 'Model removed from catalog.',
    deleteFailed: 'Failed to delete model.',
    selectionUpdated: 'Model selection updated.',
    selectionUpdateFailed: 'Failed to update model selection.',
  },
  error: {
    taskEndedWithStatus: 'Task {{taskId}} ended with status: {{status}}',
    downloadTimedOut: 'Model download timed out',
    missingDownloadedPath: 'Model download completed, but local_path is empty',
    startDownloadFailed: 'Failed to start model download task',
    onlySlabPacks: 'Only .slab model packs are supported.',
  },
} as const;
