export const setup = {
  header: {
    title: 'Setup',
    subtitle: 'Initialize local runtime dependencies',
  },
  checking: {
    title: 'Checking desktop environment',
    description: 'Inspecting the local Slab host, the packaged runtime, and FFmpeg availability.',
    wait: 'Please wait a moment.',
  },
  hostError: {
    title: 'Could not reach the local host',
    hint: 'Make sure <code>slab-server</code> is running, then reload this page.',
    reload: 'Reload',
  },
  errors: {
    failedBeforeFinish: 'Setup failed before provisioning could finish.',
  },
  badge: {
    running: 'Running',
    failed: 'Failed',
    complete: 'Complete',
  },
  hero: {
    eyebrow: 'Desktop Setup',
    titleBundled: 'Slab is checking your local tools.',
    titlePackaged: 'Slab is preparing your local runtime.',
    descriptionBundled:
      'This installation includes the runtime payload under <code>resources/libs</code>. Slab is checking FFmpeg and confirming that the managed runtime workers can start.',
    descriptionMacosMissing:
      'This macOS build expects runtime libraries under <code>resources/libs</code>. Slab is checking the bundled files before starting the local workers.',
    descriptionPackaged:
      'Slab will reuse the release CAB payloads for your current version, verify them against the embedded manifest, install the runtime into <code>resources/libs</code>, check FFmpeg, and restart the managed runtime workers.',
  },
  metrics: {
    runtimePayload: {
      label: 'Runtime Payload',
      installed: 'Installed locally',
      missingBundled: 'Checking bundle',
      needsSetup: 'Needs setup',
    },
    ffmpeg: {
      label: 'FFmpeg',
      available: 'Available',
      willBeInstalled: 'Will be installed',
    },
    backendWorkers: {
      label: 'Backend Workers',
      ready: '{{ready}}/{{total}} ready',
      notReported: 'Not reported',
    },
  },
  currentStage: 'Current Stage',
  progressHint: {
    activePackaged: 'Keep this window open while the local setup task finishes provisioning the runtime.',
    activeBundled: 'Keep this window open while Slab checks FFmpeg and confirms the bundled runtime is ready.',
    succeeded: 'Setup has completed. Slab will enter the application automatically.',
  },
  actions: {
    retry: 'Retry setup',
    launching: 'Launching Slab...',
    checkingPrerequisites: 'Checking desktop prerequisites',
    provisioning: 'Provisioning in progress',
  },
  stages: {
    failed: 'Setup failed',
    finished: 'Setup finished',
    checkingPrerequisites: 'Checking desktop prerequisites',
    starting: 'Starting setup',
    verifyingInstalledRuntime: 'Verifying installed runtime',
    preparingRuntime: 'Preparing Slab runtime',
    checkingEnvironment: 'Checking desktop environment',
    preparingEnvironment: 'Preparing environment',
  },
  hints: {
    step: 'Step {{step}} of {{count}}',
    failedBundled: 'Review the error below, then retry the local prerequisite check.',
    failedPackaged: 'Review the error below, then retry the setup task.',
    succeededBundled: 'FFmpeg and local runtime checks are complete. Launching Slab now.',
    succeededPackaged: 'Runtime payloads are in place. Launching Slab now.',
    startingBundled: 'Inspecting the installed runtime and checking whether FFmpeg is already available.',
    startingPackaged: 'Creating the setup task and connecting to the local host.',
    runningBundled: 'Checking FFmpeg runtime availability and confirming local workers are ready.',
    runningPackaged: 'Downloading payloads, verifying CABs, checking FFmpeg, and restarting runtime workers.',
    idleBundled: 'Inspecting the local desktop installation and FFmpeg availability.',
    idlePackaged: 'Inspecting the local desktop installation.',
  },
  summary: {
    failedBundled: 'Desktop prerequisite checks stopped before setup could complete.',
    failedPackaged: 'Provisioning stopped before setup could complete.',
    complete: '100% complete',
    percentComplete: '{{percentage}}% complete',
    stage: 'Stage {{step}}/{{count}}',
    startingBundled: 'Checking installed runtime...',
    startingPackaged: 'Creating setup task...',
    runningBundled: 'Checking FFmpeg and local workers...',
    runningPackaged: 'Waiting for progress updates...',
    idle: 'Waiting to begin',
  },
} as const;
