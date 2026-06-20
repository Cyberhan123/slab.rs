use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum ServerI18nKey {
    #[serde(rename = "server.errors.notFound")]
    ErrorNotFound,
    #[serde(rename = "server.errors.badRequest")]
    ErrorBadRequest,
    #[serde(rename = "server.errors.forbidden")]
    ErrorForbidden,
    #[serde(rename = "server.errors.conflict")]
    ErrorConflict,
    #[serde(rename = "server.errors.backendNotReady")]
    ErrorBackendNotReady,
    #[serde(rename = "server.errors.runtimeBusy")]
    ErrorRuntimeBusy,
    #[serde(rename = "server.errors.runtimeUnavailable")]
    ErrorRuntimeUnavailable,
    #[serde(rename = "server.errors.runtimeUnsupportedOperation")]
    ErrorRuntimeUnsupportedOperation,
    #[serde(rename = "server.errors.runtimeDriverNotRegistered")]
    ErrorRuntimeDriverNotRegistered,
    #[serde(rename = "server.errors.runtimeError")]
    ErrorRuntimeError,
    #[serde(rename = "server.errors.databaseError")]
    ErrorDatabaseError,
    #[serde(rename = "server.errors.internalError")]
    ErrorInternalError,
    #[serde(rename = "server.errors.notImplemented")]
    ErrorNotImplemented,
    #[serde(rename = "server.errors.tooManyRequests")]
    ErrorTooManyRequests,
    #[serde(rename = "server.errors.requestValidationFailed")]
    ErrorRequestValidationFailed,
    #[serde(rename = "server.agent.errors.invalidMessage")]
    AgentInvalidMessage,
    #[serde(rename = "server.tasks.setup.selectingRuntimePayload")]
    TaskSetupSelectingRuntimePayload,
    #[serde(rename = "server.tasks.setup.usingInstalledRuntimePayload")]
    TaskSetupUsingInstalledRuntimePayload,
    #[serde(rename = "server.tasks.setup.expandingRuntimePayload")]
    TaskSetupExpandingRuntimePayload,
    #[serde(rename = "server.tasks.setup.installingRuntimeLibraries")]
    TaskSetupInstallingRuntimeLibraries,
    #[serde(rename = "server.tasks.setup.checkingFfmpeg")]
    TaskSetupCheckingFfmpeg,
    #[serde(rename = "server.tasks.setup.restartingRuntimeWorkers")]
    TaskSetupRestartingRuntimeWorkers,
    #[serde(rename = "server.tasks.setup.usingCachedPayload")]
    TaskSetupUsingCachedPayload,
    #[serde(rename = "server.tasks.setup.downloadingPayload")]
    TaskSetupDownloadingPayload,
    #[serde(rename = "server.tasks.setup.downloadedPayload")]
    TaskSetupDownloadedPayload,
    #[serde(rename = "server.tasks.setup.failedBeforeFinish")]
    TaskSetupFailedBeforeFinish,
    #[serde(rename = "server.tasks.ffmpeg.audioExtraction")]
    TaskFfmpegAudioExtraction,
    #[serde(rename = "server.tasks.ffmpeg.starting")]
    TaskFfmpegStarting,
    #[serde(rename = "server.tasks.ffmpeg.completed")]
    TaskFfmpegCompleted,
    #[serde(rename = "server.tasks.ffmpeg.unsupportedOutputFormat")]
    TaskFfmpegUnsupportedOutputFormat,
    #[serde(rename = "server.tasks.ffmpeg.conversionFailed")]
    TaskFfmpegConversionFailed,
    #[serde(rename = "server.tasks.ffmpeg.workerFailed")]
    TaskFfmpegWorkerFailed,
    #[serde(rename = "server.tasks.ffmpeg.remuxCompleted")]
    TaskFfmpegRemuxCompleted,
    #[serde(rename = "server.tasks.ffmpeg.remuxFailed")]
    TaskFfmpegRemuxFailed,
    #[serde(rename = "server.tasks.ffmpeg.runtimeInitFailed")]
    TaskFfmpegRuntimeInitFailed,
    #[serde(rename = "server.settings.sections.general.title")]
    SettingsSectionGeneralTitle,
    #[serde(rename = "server.settings.sections.general.description")]
    SettingsSectionGeneralDescription,
    #[serde(rename = "server.settings.sections.database.title")]
    SettingsSectionDatabaseTitle,
    #[serde(rename = "server.settings.sections.database.description")]
    SettingsSectionDatabaseDescription,
    #[serde(rename = "server.settings.sections.logging.title")]
    SettingsSectionLoggingTitle,
    #[serde(rename = "server.settings.sections.logging.description")]
    SettingsSectionLoggingDescription,
    #[serde(rename = "server.settings.sections.telemetry.title")]
    SettingsSectionTelemetryTitle,
    #[serde(rename = "server.settings.sections.telemetry.description")]
    SettingsSectionTelemetryDescription,
    #[serde(rename = "server.settings.sections.tools.title")]
    SettingsSectionToolsTitle,
    #[serde(rename = "server.settings.sections.tools.description")]
    SettingsSectionToolsDescription,
    #[serde(rename = "server.settings.sections.runtime.title")]
    SettingsSectionRuntimeTitle,
    #[serde(rename = "server.settings.sections.runtime.description")]
    SettingsSectionRuntimeDescription,
    #[serde(rename = "server.settings.sections.providers.title")]
    SettingsSectionProvidersTitle,
    #[serde(rename = "server.settings.sections.providers.description")]
    SettingsSectionProvidersDescription,
    #[serde(rename = "server.settings.sections.models.title")]
    SettingsSectionModelsTitle,
    #[serde(rename = "server.settings.sections.models.description")]
    SettingsSectionModelsDescription,
    #[serde(rename = "server.settings.sections.plugin.title")]
    SettingsSectionPluginTitle,
    #[serde(rename = "server.settings.sections.plugin.description")]
    SettingsSectionPluginDescription,
    #[serde(rename = "server.settings.sections.agent.title")]
    SettingsSectionAgentTitle,
    #[serde(rename = "server.settings.sections.agent.description")]
    SettingsSectionAgentDescription,
    #[serde(rename = "server.settings.sections.server.title")]
    SettingsSectionServerTitle,
    #[serde(rename = "server.settings.sections.server.description")]
    SettingsSectionServerDescription,
    #[serde(rename = "server.settings.subsections.general.general.title")]
    SettingsSubsectionGeneralGeneralTitle,
    #[serde(rename = "server.settings.subsections.general.general.description")]
    SettingsSubsectionGeneralGeneralDescription,
    #[serde(rename = "server.settings.subsections.database.general.description")]
    SettingsSubsectionDatabaseGeneralDescription,
    #[serde(rename = "server.settings.subsections.logging.general.description")]
    SettingsSubsectionLoggingGeneralDescription,
    #[serde(rename = "server.settings.subsections.telemetry.general.description")]
    SettingsSubsectionTelemetryGeneralDescription,
    #[serde(rename = "server.settings.subsections.tools.ffmpeg.title")]
    SettingsSubsectionToolsFfmpegTitle,
    #[serde(rename = "server.settings.subsections.tools.ffmpeg.description")]
    SettingsSubsectionToolsFfmpegDescription,
    #[serde(rename = "server.settings.subsections.runtime.general.description")]
    SettingsSubsectionRuntimeGeneralDescription,
    #[serde(rename = "server.settings.subsections.runtime.ggml.title")]
    SettingsSubsectionRuntimeGgmlTitle,
    #[serde(rename = "server.settings.subsections.runtime.ggml.description")]
    SettingsSubsectionRuntimeGgmlDescription,
    #[serde(rename = "server.settings.subsections.runtime.llama.title")]
    SettingsSubsectionRuntimeLlamaTitle,
    #[serde(rename = "server.settings.subsections.runtime.llama.description")]
    SettingsSubsectionRuntimeLlamaDescription,
    #[serde(rename = "server.settings.subsections.runtime.whisper.title")]
    SettingsSubsectionRuntimeWhisperTitle,
    #[serde(rename = "server.settings.subsections.runtime.whisper.description")]
    SettingsSubsectionRuntimeWhisperDescription,
    #[serde(rename = "server.settings.subsections.runtime.diffusion.title")]
    SettingsSubsectionRuntimeDiffusionTitle,
    #[serde(rename = "server.settings.subsections.runtime.diffusion.description")]
    SettingsSubsectionRuntimeDiffusionDescription,
    #[serde(rename = "server.settings.subsections.runtime.candle.title")]
    SettingsSubsectionRuntimeCandleTitle,
    #[serde(rename = "server.settings.subsections.runtime.candle.description")]
    SettingsSubsectionRuntimeCandleDescription,
    #[serde(rename = "server.settings.subsections.runtime.onnx.title")]
    SettingsSubsectionRuntimeOnnxTitle,
    #[serde(rename = "server.settings.subsections.runtime.onnx.description")]
    SettingsSubsectionRuntimeOnnxDescription,
    #[serde(rename = "server.settings.subsections.providers.registry.title")]
    SettingsSubsectionProvidersRegistryTitle,
    #[serde(rename = "server.settings.subsections.providers.registry.description")]
    SettingsSubsectionProvidersRegistryDescription,
    #[serde(rename = "server.settings.subsections.models.general.description")]
    SettingsSubsectionModelsGeneralDescription,
    #[serde(rename = "server.settings.subsections.models.autoUnload.title")]
    SettingsSubsectionModelsAutoUnloadTitle,
    #[serde(rename = "server.settings.subsections.models.autoUnload.description")]
    SettingsSubsectionModelsAutoUnloadDescription,
    #[serde(rename = "server.settings.subsections.plugin.general.description")]
    SettingsSubsectionPluginGeneralDescription,
    #[serde(rename = "server.settings.subsections.agent.general.description")]
    SettingsSubsectionAgentGeneralDescription,
    #[serde(rename = "server.settings.subsections.agent.mcp.title")]
    SettingsSubsectionAgentMcpTitle,
    #[serde(rename = "server.settings.subsections.agent.mcp.description")]
    SettingsSubsectionAgentMcpDescription,
    #[serde(rename = "server.settings.subsections.agent.websearch.title")]
    SettingsSubsectionAgentWebsearchTitle,
    #[serde(rename = "server.settings.subsections.agent.websearch.description")]
    SettingsSubsectionAgentWebsearchDescription,
    #[serde(rename = "server.settings.subsections.agent.hooks.title")]
    SettingsSubsectionAgentHooksTitle,
    #[serde(rename = "server.settings.subsections.agent.hooks.description")]
    SettingsSubsectionAgentHooksDescription,
    #[serde(rename = "server.settings.subsections.agent.memories.title")]
    SettingsSubsectionAgentMemoriesTitle,
    #[serde(rename = "server.settings.subsections.agent.memories.description")]
    SettingsSubsectionAgentMemoriesDescription,
    #[serde(rename = "server.settings.subsections.server.general.description")]
    SettingsSubsectionServerGeneralDescription,
    #[serde(rename = "server.settings.subsections.server.cors.title")]
    SettingsSubsectionServerCorsTitle,
    #[serde(rename = "server.settings.subsections.server.cors.description")]
    SettingsSubsectionServerCorsDescription,
    #[serde(rename = "server.settings.subsections.server.admin.title")]
    SettingsSubsectionServerAdminTitle,
    #[serde(rename = "server.settings.subsections.server.admin.description")]
    SettingsSubsectionServerAdminDescription,
    #[serde(rename = "server.settings.subsections.server.swagger.title")]
    SettingsSubsectionServerSwaggerTitle,
    #[serde(rename = "server.settings.subsections.server.swagger.description")]
    SettingsSubsectionServerSwaggerDescription,
    #[serde(rename = "server.settings.properties.label.interfaceLanguage")]
    SettingsPropertyLabelInterfaceLanguage,
    #[serde(rename = "server.settings.properties.label.databaseUrl")]
    SettingsPropertyLabelDatabaseUrl,
    #[serde(rename = "server.settings.properties.label.logLevel")]
    SettingsPropertyLabelLogLevel,
    #[serde(rename = "server.settings.properties.label.jsonLogs")]
    SettingsPropertyLabelJsonLogs,
    #[serde(rename = "server.settings.properties.label.logDirectory")]
    SettingsPropertyLabelLogDirectory,
    #[serde(rename = "server.settings.properties.label.telemetry")]
    SettingsPropertyLabelTelemetry,
    #[serde(rename = "server.settings.properties.label.environment")]
    SettingsPropertyLabelEnvironment,
    #[serde(rename = "server.settings.properties.label.serviceName")]
    SettingsPropertyLabelServiceName,
    #[serde(rename = "server.settings.properties.label.serviceVersion")]
    SettingsPropertyLabelServiceVersion,
    #[serde(rename = "server.settings.properties.label.metricsExporter")]
    SettingsPropertyLabelMetricsExporter,
    #[serde(rename = "server.settings.properties.label.captureGenaiContent")]
    SettingsPropertyLabelCaptureGenaiContent,
    #[serde(rename = "server.settings.properties.label.spanAttributes")]
    SettingsPropertyLabelSpanAttributes,
    #[serde(rename = "server.settings.properties.label.traceState")]
    SettingsPropertyLabelTraceState,
    #[serde(rename = "server.settings.properties.label.runtimeMode")]
    SettingsPropertyLabelRuntimeMode,
    #[serde(rename = "server.settings.properties.label.transport")]
    SettingsPropertyLabelTransport,
    #[serde(rename = "server.settings.properties.label.sessionStateDirectory")]
    SettingsPropertyLabelSessionStateDirectory,
    #[serde(rename = "server.settings.properties.label.agentDebugTrace")]
    SettingsPropertyLabelAgentDebugTrace,
    #[serde(rename = "server.settings.properties.label.externalHooks")]
    SettingsPropertyLabelExternalHooks,
    #[serde(rename = "server.settings.properties.label.legacyHookScripts")]
    SettingsPropertyLabelLegacyHookScripts,
    #[serde(rename = "server.settings.properties.label.agentMemories")]
    SettingsPropertyLabelAgentMemories,
    #[serde(rename = "server.settings.properties.label.memoryRoot")]
    SettingsPropertyLabelMemoryRoot,
    #[serde(rename = "server.settings.properties.label.phase1ScanLimit")]
    SettingsPropertyLabelPhase1ScanLimit,
    #[serde(rename = "server.settings.properties.label.phase1Concurrency")]
    SettingsPropertyLabelPhase1Concurrency,
    #[serde(rename = "server.settings.properties.label.phase1IdleSeconds")]
    SettingsPropertyLabelPhase1IdleSeconds,
    #[serde(rename = "server.settings.properties.label.phase1LeaseSeconds")]
    SettingsPropertyLabelPhase1LeaseSeconds,
    #[serde(rename = "server.settings.properties.label.phase1RetrySeconds")]
    SettingsPropertyLabelPhase1RetrySeconds,
    #[serde(rename = "server.settings.properties.label.phase1MaxAgeDays")]
    SettingsPropertyLabelPhase1MaxAgeDays,
    #[serde(rename = "server.settings.properties.label.phase2Limit")]
    SettingsPropertyLabelPhase2Limit,
    #[serde(rename = "server.settings.properties.label.phase2LeaseSeconds")]
    SettingsPropertyLabelPhase2LeaseSeconds,
    #[serde(rename = "server.settings.properties.label.maxUnusedDays")]
    SettingsPropertyLabelMaxUnusedDays,
    #[serde(rename = "server.settings.properties.label.extensionRetentionDays")]
    SettingsPropertyLabelExtensionRetentionDays,
    #[serde(rename = "server.settings.properties.label.mcpTools")]
    SettingsPropertyLabelMcpTools,
    #[serde(rename = "server.settings.properties.label.defaultProvider")]
    SettingsPropertyLabelDefaultProvider,
    #[serde(rename = "server.settings.properties.label.webSearchProviders")]
    SettingsPropertyLabelWebSearchProviders,
    #[serde(rename = "server.settings.properties.label.flashAttention")]
    SettingsPropertyLabelFlashAttention,
    #[serde(rename = "server.settings.properties.label.providerRegistry")]
    SettingsPropertyLabelProviderRegistry,
    #[serde(rename = "server.settings.properties.label.modelCacheDirectory")]
    SettingsPropertyLabelModelCacheDirectory,
    #[serde(rename = "server.settings.properties.label.modelConfigDirectory")]
    SettingsPropertyLabelModelConfigDirectory,
    #[serde(rename = "server.settings.properties.label.modelSource")]
    SettingsPropertyLabelModelSource,
    #[serde(rename = "server.settings.properties.label.pluginInstallDirectory")]
    SettingsPropertyLabelPluginInstallDirectory,
    #[serde(rename = "server.settings.properties.label.jsRuntimeTransport")]
    SettingsPropertyLabelJsRuntimeTransport,
    #[serde(rename = "server.settings.properties.label.pythonRuntimeTransport")]
    SettingsPropertyLabelPythonRuntimeTransport,
    #[serde(rename = "server.settings.properties.label.bindAddress")]
    SettingsPropertyLabelBindAddress,
    #[serde(rename = "server.settings.properties.label.adminToken")]
    SettingsPropertyLabelAdminToken,
    #[serde(rename = "server.settings.properties.label.allowedOrigins")]
    SettingsPropertyLabelAllowedOrigins,
    #[serde(rename = "server.settings.properties.label.cloudHttpTrace")]
    SettingsPropertyLabelCloudHttpTrace,
    #[serde(rename = "server.settings.properties.label.agentMemoryModel")]
    SettingsPropertyLabelAgentMemoryModel,
    #[serde(rename = "server.settings.properties.label.mcpServers")]
    SettingsPropertyLabelMcpServers,
    #[serde(rename = "server.settings.properties.label.autoUnloadIdleMinutes")]
    SettingsPropertyLabelAutoUnloadIdleMinutes,
    #[serde(rename = "server.settings.properties.label.autoUnloadMinFreeSystemMemoryBytes")]
    SettingsPropertyLabelAutoUnloadMinFreeSystemMemoryBytes,
    #[serde(rename = "server.settings.properties.label.autoUnloadMinFreeGpuMemoryBytes")]
    SettingsPropertyLabelAutoUnloadMinFreeGpuMemoryBytes,
    #[serde(rename = "server.settings.properties.label.autoUnloadMaxPressureEvictionsPerLoad")]
    SettingsPropertyLabelAutoUnloadMaxPressureEvictionsPerLoad,
    #[serde(rename = "server.settings.properties.label.genericEnabled")]
    SettingsPropertyLabelGenericEnabled,
    #[serde(rename = "server.settings.properties.label.genericAutoDownload")]
    SettingsPropertyLabelGenericAutoDownload,
    #[serde(rename = "server.settings.properties.label.genericInstallDirectory")]
    SettingsPropertyLabelGenericInstallDirectory,
    #[serde(rename = "server.settings.properties.label.genericLogLevel")]
    SettingsPropertyLabelGenericLogLevel,
    #[serde(rename = "server.settings.properties.label.genericJsonLogs")]
    SettingsPropertyLabelGenericJsonLogs,
    #[serde(rename = "server.settings.properties.label.genericPath")]
    SettingsPropertyLabelGenericPath,
    #[serde(rename = "server.settings.properties.label.genericQueue")]
    SettingsPropertyLabelGenericQueue,
    #[serde(rename = "server.settings.properties.label.genericConcurrentRequests")]
    SettingsPropertyLabelGenericConcurrentRequests,
    #[serde(rename = "server.settings.properties.label.genericAddress")]
    SettingsPropertyLabelGenericAddress,
    #[serde(rename = "server.settings.properties.label.genericIpcPath")]
    SettingsPropertyLabelGenericIpcPath,
    #[serde(rename = "server.settings.properties.label.genericVersion")]
    SettingsPropertyLabelGenericVersion,
    #[serde(rename = "server.settings.properties.label.genericArtifact")]
    SettingsPropertyLabelGenericArtifact,
    #[serde(rename = "server.settings.properties.label.genericContextLength")]
    SettingsPropertyLabelGenericContextLength,
    #[serde(rename = "server.settings.properties.description.interfaceLanguage")]
    SettingsPropertyDescriptionInterfaceLanguage,
    #[serde(rename = "server.settings.properties.description.databaseUrl")]
    SettingsPropertyDescriptionDatabaseUrl,
    #[serde(rename = "server.settings.properties.description.logLevel")]
    SettingsPropertyDescriptionLogLevel,
    #[serde(rename = "server.settings.properties.description.jsonLogs")]
    SettingsPropertyDescriptionJsonLogs,
    #[serde(rename = "server.settings.properties.description.logDirectory")]
    SettingsPropertyDescriptionLogDirectory,
    #[serde(rename = "server.settings.properties.description.telemetry")]
    SettingsPropertyDescriptionTelemetry,
    #[serde(rename = "server.settings.properties.description.environment")]
    SettingsPropertyDescriptionEnvironment,
    #[serde(rename = "server.settings.properties.description.serviceName")]
    SettingsPropertyDescriptionServiceName,
    #[serde(rename = "server.settings.properties.description.serviceVersion")]
    SettingsPropertyDescriptionServiceVersion,
    #[serde(rename = "server.settings.properties.description.metricsExporter")]
    SettingsPropertyDescriptionMetricsExporter,
    #[serde(rename = "server.settings.properties.description.captureGenaiContent")]
    SettingsPropertyDescriptionCaptureGenaiContent,
    #[serde(rename = "server.settings.properties.description.spanAttributes")]
    SettingsPropertyDescriptionSpanAttributes,
    #[serde(rename = "server.settings.properties.description.traceState")]
    SettingsPropertyDescriptionTraceState,
    #[serde(rename = "server.settings.properties.description.ffmpegEnabled")]
    SettingsPropertyDescriptionFfmpegEnabled,
    #[serde(rename = "server.settings.properties.description.ffmpegAutoDownload")]
    SettingsPropertyDescriptionFfmpegAutoDownload,
    #[serde(rename = "server.settings.properties.description.ffmpegInstallDir")]
    SettingsPropertyDescriptionFfmpegInstallDir,
    #[serde(rename = "server.settings.properties.description.agentDebugTrace")]
    SettingsPropertyDescriptionAgentDebugTrace,
    #[serde(rename = "server.settings.properties.description.externalHooks")]
    SettingsPropertyDescriptionExternalHooks,
    #[serde(rename = "server.settings.properties.description.legacyHookScripts")]
    SettingsPropertyDescriptionLegacyHookScripts,
    #[serde(rename = "server.settings.properties.description.agentMemories")]
    SettingsPropertyDescriptionAgentMemories,
    #[serde(rename = "server.settings.properties.description.agentMemoryModel")]
    SettingsPropertyDescriptionAgentMemoryModel,
    #[serde(rename = "server.settings.properties.description.memoryRoot")]
    SettingsPropertyDescriptionMemoryRoot,
    #[serde(rename = "server.settings.properties.description.phase1ScanLimit")]
    SettingsPropertyDescriptionPhase1ScanLimit,
    #[serde(rename = "server.settings.properties.description.phase1Concurrency")]
    SettingsPropertyDescriptionPhase1Concurrency,
    #[serde(rename = "server.settings.properties.description.phase1IdleSeconds")]
    SettingsPropertyDescriptionPhase1IdleSeconds,
    #[serde(rename = "server.settings.properties.description.phase1LeaseSeconds")]
    SettingsPropertyDescriptionPhase1LeaseSeconds,
    #[serde(rename = "server.settings.properties.description.phase1RetrySeconds")]
    SettingsPropertyDescriptionPhase1RetrySeconds,
    #[serde(rename = "server.settings.properties.description.phase1MaxAgeDays")]
    SettingsPropertyDescriptionPhase1MaxAgeDays,
    #[serde(rename = "server.settings.properties.description.phase2Limit")]
    SettingsPropertyDescriptionPhase2Limit,
    #[serde(rename = "server.settings.properties.description.phase2LeaseSeconds")]
    SettingsPropertyDescriptionPhase2LeaseSeconds,
    #[serde(rename = "server.settings.properties.description.maxUnusedDays")]
    SettingsPropertyDescriptionMaxUnusedDays,
    #[serde(rename = "server.settings.properties.description.extensionRetentionDays")]
    SettingsPropertyDescriptionExtensionRetentionDays,
    #[serde(rename = "server.settings.properties.description.mcpTools")]
    SettingsPropertyDescriptionMcpTools,
    #[serde(rename = "server.settings.properties.description.mcpServers")]
    SettingsPropertyDescriptionMcpServers,
    #[serde(rename = "server.settings.properties.description.defaultProvider")]
    SettingsPropertyDescriptionDefaultProvider,
    #[serde(rename = "server.settings.properties.description.webSearchProviders")]
    SettingsPropertyDescriptionWebSearchProviders,
    #[serde(rename = "server.settings.properties.description.runtimeMode")]
    SettingsPropertyDescriptionRuntimeMode,
    #[serde(rename = "server.settings.properties.description.runtimeTransport")]
    SettingsPropertyDescriptionRuntimeTransport,
    #[serde(rename = "server.settings.properties.description.sessionStateDirectory")]
    SettingsPropertyDescriptionSessionStateDirectory,
    #[serde(rename = "server.settings.properties.description.providerRegistry")]
    SettingsPropertyDescriptionProviderRegistry,
    #[serde(rename = "server.settings.properties.description.modelCacheDirectory")]
    SettingsPropertyDescriptionModelCacheDirectory,
    #[serde(rename = "server.settings.properties.description.modelConfigDirectory")]
    SettingsPropertyDescriptionModelConfigDirectory,
    #[serde(rename = "server.settings.properties.description.modelSource")]
    SettingsPropertyDescriptionModelSource,
    #[serde(rename = "server.settings.properties.description.pluginInstallDirectory")]
    SettingsPropertyDescriptionPluginInstallDirectory,
    #[serde(rename = "server.settings.properties.description.jsRuntimeTransport")]
    SettingsPropertyDescriptionJsRuntimeTransport,
    #[serde(rename = "server.settings.properties.description.pythonRuntimeTransport")]
    SettingsPropertyDescriptionPythonRuntimeTransport,
    #[serde(rename = "server.settings.properties.description.autoUnloadEnabled")]
    SettingsPropertyDescriptionAutoUnloadEnabled,
    #[serde(rename = "server.settings.properties.description.autoUnloadIdleMinutes")]
    SettingsPropertyDescriptionAutoUnloadIdleMinutes,
    #[serde(rename = "server.settings.properties.description.autoUnloadMinFreeSystemMemoryBytes")]
    SettingsPropertyDescriptionAutoUnloadMinFreeSystemMemoryBytes,
    #[serde(rename = "server.settings.properties.description.autoUnloadMinFreeGpuMemoryBytes")]
    SettingsPropertyDescriptionAutoUnloadMinFreeGpuMemoryBytes,
    #[serde(
        rename = "server.settings.properties.description.autoUnloadMaxPressureEvictionsPerLoad"
    )]
    SettingsPropertyDescriptionAutoUnloadMaxPressureEvictionsPerLoad,
    #[serde(rename = "server.settings.properties.description.serverAddress")]
    SettingsPropertyDescriptionServerAddress,
    #[serde(rename = "server.settings.properties.description.adminToken")]
    SettingsPropertyDescriptionAdminToken,
    #[serde(rename = "server.settings.properties.description.allowedOrigins")]
    SettingsPropertyDescriptionAllowedOrigins,
    #[serde(rename = "server.settings.properties.description.swaggerEnabled")]
    SettingsPropertyDescriptionSwaggerEnabled,
    #[serde(rename = "server.settings.properties.description.cloudHttpTrace")]
    SettingsPropertyDescriptionCloudHttpTrace,
    #[serde(rename = "server.settings.properties.description.genericEnabled")]
    SettingsPropertyDescriptionGenericEnabled,
    #[serde(rename = "server.settings.properties.description.genericFlashAttention")]
    SettingsPropertyDescriptionGenericFlashAttention,
    #[serde(rename = "server.settings.properties.description.genericInstallDirectory")]
    SettingsPropertyDescriptionGenericInstallDirectory,
    #[serde(rename = "server.settings.properties.description.genericLogLevel")]
    SettingsPropertyDescriptionGenericLogLevel,
    #[serde(rename = "server.settings.properties.description.genericJsonLogs")]
    SettingsPropertyDescriptionGenericJsonLogs,
    #[serde(rename = "server.settings.properties.description.genericPath")]
    SettingsPropertyDescriptionGenericPath,
    #[serde(rename = "server.settings.properties.description.genericQueue")]
    SettingsPropertyDescriptionGenericQueue,
    #[serde(rename = "server.settings.properties.description.genericConcurrentRequests")]
    SettingsPropertyDescriptionGenericConcurrentRequests,
    #[serde(rename = "server.settings.properties.description.genericAddress")]
    SettingsPropertyDescriptionGenericAddress,
    #[serde(rename = "server.settings.properties.description.genericIpcPath")]
    SettingsPropertyDescriptionGenericIpcPath,
    #[serde(rename = "server.settings.properties.description.genericVersion")]
    SettingsPropertyDescriptionGenericVersion,
    #[serde(rename = "server.settings.properties.description.genericArtifact")]
    SettingsPropertyDescriptionGenericArtifact,
    #[serde(rename = "server.settings.properties.description.genericContextLength")]
    SettingsPropertyDescriptionGenericContextLength,
    #[serde(rename = "server.settings.schemas.provider.entry.title")]
    SettingsSchemaProviderEntryTitle,
    #[serde(rename = "server.settings.schemas.provider.id.title")]
    SettingsSchemaProviderIdTitle,
    #[serde(rename = "server.settings.schemas.provider.id.description")]
    SettingsSchemaProviderIdDescription,
    #[serde(rename = "server.settings.schemas.provider.family.title")]
    SettingsSchemaProviderFamilyTitle,
    #[serde(rename = "server.settings.schemas.provider.displayName.title")]
    SettingsSchemaProviderDisplayNameTitle,
    #[serde(rename = "server.settings.schemas.provider.apiBase.title")]
    SettingsSchemaProviderApiBaseTitle,
    #[serde(rename = "server.settings.schemas.provider.auth.title")]
    SettingsSchemaProviderAuthTitle,
    #[serde(rename = "server.settings.schemas.provider.apiKey.title")]
    SettingsSchemaProviderApiKeyTitle,
    #[serde(rename = "server.settings.schemas.provider.apiKeyEnv.title")]
    SettingsSchemaProviderApiKeyEnvTitle,
    #[serde(rename = "server.settings.schemas.provider.requestDefaults.title")]
    SettingsSchemaProviderRequestDefaultsTitle,
    #[serde(rename = "server.settings.schemas.provider.headers.title")]
    SettingsSchemaProviderHeadersTitle,
    #[serde(rename = "server.settings.schemas.provider.query.title")]
    SettingsSchemaProviderQueryTitle,
    #[serde(rename = "server.settings.schemas.webSearch.baseUrl.title")]
    SettingsSchemaWebSearchBaseUrlTitle,
    #[serde(rename = "server.settings.schemas.webSearch.userAgent.title")]
    SettingsSchemaWebSearchUserAgentTitle,
    #[serde(rename = "server.settings.schemas.webSearch.useLite.title")]
    SettingsSchemaWebSearchUseLiteTitle,
    #[serde(rename = "server.settings.schemas.webSearch.searchEngineId.title")]
    SettingsSchemaWebSearchSearchEngineIdTitle,
    #[serde(rename = "server.settings.schemas.webSearch.searchDepth.title")]
    SettingsSchemaWebSearchSearchDepthTitle,
    #[serde(rename = "server.settings.schemas.webSearch.includeAnswer.title")]
    SettingsSchemaWebSearchIncludeAnswerTitle,
    #[serde(rename = "server.settings.schemas.webSearch.includeImages.title")]
    SettingsSchemaWebSearchIncludeImagesTitle,
    #[serde(rename = "server.settings.schemas.webSearch.includeRawContent.title")]
    SettingsSchemaWebSearchIncludeRawContentTitle,
    #[serde(rename = "server.settings.schemas.webSearch.includeContents.title")]
    SettingsSchemaWebSearchIncludeContentsTitle,
    #[serde(rename = "server.settings.schemas.webSearch.engine.title")]
    SettingsSchemaWebSearchEngineTitle,
    #[serde(rename = "server.settings.schemas.webSearch.model.title")]
    SettingsSchemaWebSearchModelTitle,
    #[serde(rename = "server.settings.schemas.mcp.server.title")]
    SettingsSchemaMcpServerTitle,
    #[serde(rename = "server.settings.schemas.mcp.enabled.title")]
    SettingsSchemaMcpEnabledTitle,
    #[serde(rename = "server.settings.schemas.mcp.name.title")]
    SettingsSchemaMcpNameTitle,
    #[serde(rename = "server.settings.schemas.mcp.name.description")]
    SettingsSchemaMcpNameDescription,
    #[serde(rename = "server.settings.schemas.mcp.command.title")]
    SettingsSchemaMcpCommandTitle,
    #[serde(rename = "server.settings.schemas.mcp.command.description")]
    SettingsSchemaMcpCommandDescription,
    #[serde(rename = "server.settings.schemas.mcp.args.title")]
    SettingsSchemaMcpArgsTitle,
    #[serde(rename = "server.settings.schemas.mcp.cwd.title")]
    SettingsSchemaMcpCwdTitle,
    #[serde(rename = "server.settings.schemas.mcp.env.title")]
    SettingsSchemaMcpEnvTitle,
    #[serde(rename = "server.settings.schemas.mcp.env.description")]
    SettingsSchemaMcpEnvDescription,
    #[serde(rename = "server.settings.schemas.mcp.envReference.title")]
    SettingsSchemaMcpEnvReferenceTitle,
    #[serde(rename = "server.settings.schemas.mcp.envVar.title")]
    SettingsSchemaMcpEnvVarTitle,
    #[serde(rename = "server.settings.schemas.stringEntry.title")]
    SettingsSchemaStringEntryTitle,
    #[serde(rename = "server.modelConfig.sections.summary.label")]
    ModelConfigSectionSummaryLabel,
    #[serde(rename = "server.modelConfig.sections.summary.description")]
    ModelConfigSectionSummaryDescription,
    #[serde(rename = "server.modelConfig.sections.source.label")]
    ModelConfigSectionSourceLabel,
    #[serde(rename = "server.modelConfig.sections.source.description")]
    ModelConfigSectionSourceDescription,
    #[serde(rename = "server.modelConfig.sections.load.label")]
    ModelConfigSectionLoadLabel,
    #[serde(rename = "server.modelConfig.sections.load.description")]
    ModelConfigSectionLoadDescription,
    #[serde(rename = "server.modelConfig.sections.load.nonRuntimeDescription")]
    ModelConfigSectionLoadNonRuntimeDescription,
    #[serde(rename = "server.modelConfig.sections.inference.label")]
    ModelConfigSectionInferenceLabel,
    #[serde(rename = "server.modelConfig.sections.inference.description")]
    ModelConfigSectionInferenceDescription,
    #[serde(rename = "server.modelConfig.sections.advanced.label")]
    ModelConfigSectionAdvancedLabel,
    #[serde(rename = "server.modelConfig.sections.advanced.description")]
    ModelConfigSectionAdvancedDescription,
    #[serde(rename = "server.modelConfig.sections.advanced.nonRuntimeDescription")]
    ModelConfigSectionAdvancedNonRuntimeDescription,
    #[serde(rename = "server.modelConfig.fields.modelId.label")]
    ModelConfigFieldModelIdLabel,
    #[serde(rename = "server.modelConfig.fields.modelId.description")]
    ModelConfigFieldModelIdDescription,
    #[serde(rename = "server.modelConfig.fields.displayName.label")]
    ModelConfigFieldDisplayNameLabel,
    #[serde(rename = "server.modelConfig.fields.displayName.description")]
    ModelConfigFieldDisplayNameDescription,
    #[serde(rename = "server.modelConfig.fields.backend.label")]
    ModelConfigFieldBackendLabel,
    #[serde(rename = "server.modelConfig.fields.backend.runtimeDescription")]
    ModelConfigFieldBackendRuntimeDescription,
    #[serde(rename = "server.modelConfig.fields.backend.productDescription")]
    ModelConfigFieldBackendProductDescription,
    #[serde(rename = "server.modelConfig.fields.catalogStatus.label")]
    ModelConfigFieldCatalogStatusLabel,
    #[serde(rename = "server.modelConfig.fields.catalogStatus.description")]
    ModelConfigFieldCatalogStatusDescription,
    #[serde(rename = "server.modelConfig.fields.capabilities.label")]
    ModelConfigFieldCapabilitiesLabel,
    #[serde(rename = "server.modelConfig.fields.capabilities.description")]
    ModelConfigFieldCapabilitiesDescription,
    #[serde(rename = "server.modelConfig.fields.sourceKind.label")]
    ModelConfigFieldSourceKindLabel,
    #[serde(rename = "server.modelConfig.fields.sourceKind.description")]
    ModelConfigFieldSourceKindDescription,
    #[serde(rename = "server.modelConfig.fields.repoId.label")]
    ModelConfigFieldRepoIdLabel,
    #[serde(rename = "server.modelConfig.fields.repoId.description")]
    ModelConfigFieldRepoIdDescription,
    #[serde(rename = "server.modelConfig.fields.primaryArtifact.label")]
    ModelConfigFieldPrimaryArtifactLabel,
    #[serde(rename = "server.modelConfig.fields.primaryArtifact.description")]
    ModelConfigFieldPrimaryArtifactDescription,
    #[serde(rename = "server.modelConfig.fields.localPath.label")]
    ModelConfigFieldLocalPathLabel,
    #[serde(rename = "server.modelConfig.fields.localPath.description")]
    ModelConfigFieldLocalPathDescription,
    #[serde(rename = "server.modelConfig.fields.artifactPath.description")]
    ModelConfigFieldArtifactPathDescription,
    #[serde(rename = "server.modelConfig.fields.temperature.label")]
    ModelConfigFieldTemperatureLabel,
    #[serde(rename = "server.modelConfig.fields.temperature.description")]
    ModelConfigFieldTemperatureDescription,
    #[serde(rename = "server.modelConfig.fields.topP.label")]
    ModelConfigFieldTopPLabel,
    #[serde(rename = "server.modelConfig.fields.topP.description")]
    ModelConfigFieldTopPDescription,
    #[serde(rename = "server.modelConfig.fields.workers.label")]
    ModelConfigFieldWorkersLabel,
    #[serde(rename = "server.modelConfig.fields.workers.description")]
    ModelConfigFieldWorkersDescription,
    #[serde(rename = "server.modelConfig.fields.contextLength.label")]
    ModelConfigFieldContextLengthLabel,
    #[serde(rename = "server.modelConfig.fields.contextLength.description")]
    ModelConfigFieldContextLengthDescription,
    #[serde(rename = "server.modelConfig.fields.chatTemplate.label")]
    ModelConfigFieldChatTemplateLabel,
    #[serde(rename = "server.modelConfig.fields.chatTemplate.description")]
    ModelConfigFieldChatTemplateDescription,
    #[serde(rename = "server.modelConfig.fields.gbnf.label")]
    ModelConfigFieldGbnfLabel,
    #[serde(rename = "server.modelConfig.fields.gbnf.description")]
    ModelConfigFieldGbnfDescription,
    #[serde(rename = "server.modelConfig.fields.diffusionAsset.label")]
    ModelConfigFieldDiffusionAssetLabel,
    #[serde(rename = "server.modelConfig.fields.diffusionAsset.description")]
    ModelConfigFieldDiffusionAssetDescription,
    #[serde(rename = "server.modelConfig.fields.flashAttention.label")]
    ModelConfigFieldFlashAttentionLabel,
    #[serde(rename = "server.modelConfig.fields.diffusionPerformance.description")]
    ModelConfigFieldDiffusionPerformanceDescription,
    #[serde(rename = "server.modelConfig.fields.offloadParamsToCpu.label")]
    ModelConfigFieldOffloadParamsToCpuLabel,
    #[serde(rename = "server.modelConfig.fields.diffusionDevice.description")]
    ModelConfigFieldDiffusionDeviceDescription,
    #[serde(rename = "server.modelConfig.fields.vaeDevice.label")]
    ModelConfigFieldVaeDeviceLabel,
    #[serde(rename = "server.modelConfig.fields.clipDevice.label")]
    ModelConfigFieldClipDeviceLabel,
    #[serde(rename = "server.modelConfig.fields.runtimeLoadSupported.label")]
    ModelConfigFieldRuntimeLoadSupportedLabel,
    #[serde(rename = "server.modelConfig.fields.runtimeLoadSupported.description")]
    ModelConfigFieldRuntimeLoadSupportedDescription,
    #[serde(rename = "server.modelConfig.fields.nonRuntimeProjection.label")]
    ModelConfigFieldNonRuntimeProjectionLabel,
    #[serde(rename = "server.modelConfig.fields.nonRuntimeProjection.description")]
    ModelConfigFieldNonRuntimeProjectionDescription,
    #[serde(rename = "server.modelConfig.fields.resolvedLoadJson.label")]
    ModelConfigFieldResolvedLoadJsonLabel,
    #[serde(rename = "server.modelConfig.fields.resolvedLoadJson.description")]
    ModelConfigFieldResolvedLoadJsonDescription,
    #[serde(rename = "server.modelConfig.fields.resolvedInferenceJson.label")]
    ModelConfigFieldResolvedInferenceJsonLabel,
    #[serde(rename = "server.modelConfig.fields.resolvedInferenceJson.description")]
    ModelConfigFieldResolvedInferenceJsonDescription,
    #[serde(rename = "server.modelConfig.fields.resolvedInferenceJson.nonRuntimeDescription")]
    ModelConfigFieldResolvedInferenceJsonNonRuntimeDescription,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct I18nMessageRef {
    pub key: ServerI18nKey,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Value>,
}

impl I18nMessageRef {
    pub fn new(key: ServerI18nKey) -> Self {
        Self { key, params: BTreeMap::new() }
    }

    pub fn with_params(key: ServerI18nKey, params: BTreeMap<String, Value>) -> Self {
        Self { key, params }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct I18nPayload(pub BTreeMap<String, I18nMessageRef>);

impl I18nPayload {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn with_field(field: impl Into<String>, key: ServerI18nKey) -> Self {
        let mut payload = Self::new();
        payload.insert(field, I18nMessageRef::new(key));
        payload
    }

    pub fn with_field_params(
        field: impl Into<String>,
        key: ServerI18nKey,
        params: BTreeMap<String, Value>,
    ) -> Self {
        let mut payload = Self::new();
        payload.insert(field, I18nMessageRef::with_params(key, params));
        payload
    }

    pub fn insert(&mut self, field: impl Into<String>, message: I18nMessageRef) {
        self.0.insert(field.into(), message);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
