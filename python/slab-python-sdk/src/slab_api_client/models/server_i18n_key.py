from enum import Enum


class ServerI18NKey(str, Enum):
    SERVER_AGENT_ERRORS_INVALIDMESSAGE = "server.agent.errors.invalidMessage"
    SERVER_ERRORS_BACKENDNOTREADY = "server.errors.backendNotReady"
    SERVER_ERRORS_BADREQUEST = "server.errors.badRequest"
    SERVER_ERRORS_CONFLICT = "server.errors.conflict"
    SERVER_ERRORS_DATABASEERROR = "server.errors.databaseError"
    SERVER_ERRORS_FORBIDDEN = "server.errors.forbidden"
    SERVER_ERRORS_INTERNALERROR = "server.errors.internalError"
    SERVER_ERRORS_NOTFOUND = "server.errors.notFound"
    SERVER_ERRORS_NOTIMPLEMENTED = "server.errors.notImplemented"
    SERVER_ERRORS_REQUESTVALIDATIONFAILED = "server.errors.requestValidationFailed"
    SERVER_ERRORS_RUNTIMEBUSY = "server.errors.runtimeBusy"
    SERVER_ERRORS_RUNTIMEDRIVERNOTREGISTERED = (
        "server.errors.runtimeDriverNotRegistered"
    )
    SERVER_ERRORS_RUNTIMEERROR = "server.errors.runtimeError"
    SERVER_ERRORS_RUNTIMEUNAVAILABLE = "server.errors.runtimeUnavailable"
    SERVER_ERRORS_RUNTIMEUNSUPPORTEDOPERATION = (
        "server.errors.runtimeUnsupportedOperation"
    )
    SERVER_ERRORS_TOOMANYREQUESTS = "server.errors.tooManyRequests"
    SERVER_MODELCONFIG_FIELDS_ARTIFACTPATH_DESCRIPTION = (
        "server.modelConfig.fields.artifactPath.description"
    )
    SERVER_MODELCONFIG_FIELDS_BACKEND_LABEL = "server.modelConfig.fields.backend.label"
    SERVER_MODELCONFIG_FIELDS_BACKEND_PRODUCTDESCRIPTION = (
        "server.modelConfig.fields.backend.productDescription"
    )
    SERVER_MODELCONFIG_FIELDS_BACKEND_RUNTIMEDESCRIPTION = (
        "server.modelConfig.fields.backend.runtimeDescription"
    )
    SERVER_MODELCONFIG_FIELDS_CAPABILITIES_DESCRIPTION = (
        "server.modelConfig.fields.capabilities.description"
    )
    SERVER_MODELCONFIG_FIELDS_CAPABILITIES_LABEL = (
        "server.modelConfig.fields.capabilities.label"
    )
    SERVER_MODELCONFIG_FIELDS_CATALOGSTATUS_DESCRIPTION = (
        "server.modelConfig.fields.catalogStatus.description"
    )
    SERVER_MODELCONFIG_FIELDS_CATALOGSTATUS_LABEL = (
        "server.modelConfig.fields.catalogStatus.label"
    )
    SERVER_MODELCONFIG_FIELDS_CHATTEMPLATE_DESCRIPTION = (
        "server.modelConfig.fields.chatTemplate.description"
    )
    SERVER_MODELCONFIG_FIELDS_CHATTEMPLATE_LABEL = (
        "server.modelConfig.fields.chatTemplate.label"
    )
    SERVER_MODELCONFIG_FIELDS_CLIPDEVICE_LABEL = (
        "server.modelConfig.fields.clipDevice.label"
    )
    SERVER_MODELCONFIG_FIELDS_CONTEXTLENGTH_DESCRIPTION = (
        "server.modelConfig.fields.contextLength.description"
    )
    SERVER_MODELCONFIG_FIELDS_CONTEXTLENGTH_LABEL = (
        "server.modelConfig.fields.contextLength.label"
    )
    SERVER_MODELCONFIG_FIELDS_DIFFUSIONASSET_DESCRIPTION = (
        "server.modelConfig.fields.diffusionAsset.description"
    )
    SERVER_MODELCONFIG_FIELDS_DIFFUSIONASSET_LABEL = (
        "server.modelConfig.fields.diffusionAsset.label"
    )
    SERVER_MODELCONFIG_FIELDS_DIFFUSIONDEVICE_DESCRIPTION = (
        "server.modelConfig.fields.diffusionDevice.description"
    )
    SERVER_MODELCONFIG_FIELDS_DIFFUSIONPERFORMANCE_DESCRIPTION = (
        "server.modelConfig.fields.diffusionPerformance.description"
    )
    SERVER_MODELCONFIG_FIELDS_DISPLAYNAME_DESCRIPTION = (
        "server.modelConfig.fields.displayName.description"
    )
    SERVER_MODELCONFIG_FIELDS_DISPLAYNAME_LABEL = (
        "server.modelConfig.fields.displayName.label"
    )
    SERVER_MODELCONFIG_FIELDS_FLASHATTENTION_LABEL = (
        "server.modelConfig.fields.flashAttention.label"
    )
    SERVER_MODELCONFIG_FIELDS_GBNF_DESCRIPTION = (
        "server.modelConfig.fields.gbnf.description"
    )
    SERVER_MODELCONFIG_FIELDS_GBNF_LABEL = "server.modelConfig.fields.gbnf.label"
    SERVER_MODELCONFIG_FIELDS_LOCALPATH_DESCRIPTION = (
        "server.modelConfig.fields.localPath.description"
    )
    SERVER_MODELCONFIG_FIELDS_LOCALPATH_LABEL = (
        "server.modelConfig.fields.localPath.label"
    )
    SERVER_MODELCONFIG_FIELDS_MODELID_DESCRIPTION = (
        "server.modelConfig.fields.modelId.description"
    )
    SERVER_MODELCONFIG_FIELDS_MODELID_LABEL = "server.modelConfig.fields.modelId.label"
    SERVER_MODELCONFIG_FIELDS_NONRUNTIMEPROJECTION_DESCRIPTION = (
        "server.modelConfig.fields.nonRuntimeProjection.description"
    )
    SERVER_MODELCONFIG_FIELDS_NONRUNTIMEPROJECTION_LABEL = (
        "server.modelConfig.fields.nonRuntimeProjection.label"
    )
    SERVER_MODELCONFIG_FIELDS_OFFLOADPARAMSTOCPU_LABEL = (
        "server.modelConfig.fields.offloadParamsToCpu.label"
    )
    SERVER_MODELCONFIG_FIELDS_PRIMARYARTIFACT_DESCRIPTION = (
        "server.modelConfig.fields.primaryArtifact.description"
    )
    SERVER_MODELCONFIG_FIELDS_PRIMARYARTIFACT_LABEL = (
        "server.modelConfig.fields.primaryArtifact.label"
    )
    SERVER_MODELCONFIG_FIELDS_REPOID_DESCRIPTION = (
        "server.modelConfig.fields.repoId.description"
    )
    SERVER_MODELCONFIG_FIELDS_REPOID_LABEL = "server.modelConfig.fields.repoId.label"
    SERVER_MODELCONFIG_FIELDS_RESOLVEDINFERENCEJSON_DESCRIPTION = (
        "server.modelConfig.fields.resolvedInferenceJson.description"
    )
    SERVER_MODELCONFIG_FIELDS_RESOLVEDINFERENCEJSON_LABEL = (
        "server.modelConfig.fields.resolvedInferenceJson.label"
    )
    SERVER_MODELCONFIG_FIELDS_RESOLVEDINFERENCEJSON_NONRUNTIMEDESCRIPTION = (
        "server.modelConfig.fields.resolvedInferenceJson.nonRuntimeDescription"
    )
    SERVER_MODELCONFIG_FIELDS_RESOLVEDLOADJSON_DESCRIPTION = (
        "server.modelConfig.fields.resolvedLoadJson.description"
    )
    SERVER_MODELCONFIG_FIELDS_RESOLVEDLOADJSON_LABEL = (
        "server.modelConfig.fields.resolvedLoadJson.label"
    )
    SERVER_MODELCONFIG_FIELDS_RUNTIMELOADSUPPORTED_DESCRIPTION = (
        "server.modelConfig.fields.runtimeLoadSupported.description"
    )
    SERVER_MODELCONFIG_FIELDS_RUNTIMELOADSUPPORTED_LABEL = (
        "server.modelConfig.fields.runtimeLoadSupported.label"
    )
    SERVER_MODELCONFIG_FIELDS_SOURCEKIND_DESCRIPTION = (
        "server.modelConfig.fields.sourceKind.description"
    )
    SERVER_MODELCONFIG_FIELDS_SOURCEKIND_LABEL = (
        "server.modelConfig.fields.sourceKind.label"
    )
    SERVER_MODELCONFIG_FIELDS_TEMPERATURE_DESCRIPTION = (
        "server.modelConfig.fields.temperature.description"
    )
    SERVER_MODELCONFIG_FIELDS_TEMPERATURE_LABEL = (
        "server.modelConfig.fields.temperature.label"
    )
    SERVER_MODELCONFIG_FIELDS_TOPP_DESCRIPTION = (
        "server.modelConfig.fields.topP.description"
    )
    SERVER_MODELCONFIG_FIELDS_TOPP_LABEL = "server.modelConfig.fields.topP.label"
    SERVER_MODELCONFIG_FIELDS_VAEDEVICE_LABEL = (
        "server.modelConfig.fields.vaeDevice.label"
    )
    SERVER_MODELCONFIG_FIELDS_WORKERS_DESCRIPTION = (
        "server.modelConfig.fields.workers.description"
    )
    SERVER_MODELCONFIG_FIELDS_WORKERS_LABEL = "server.modelConfig.fields.workers.label"
    SERVER_MODELCONFIG_SECTIONS_ADVANCED_DESCRIPTION = (
        "server.modelConfig.sections.advanced.description"
    )
    SERVER_MODELCONFIG_SECTIONS_ADVANCED_LABEL = (
        "server.modelConfig.sections.advanced.label"
    )
    SERVER_MODELCONFIG_SECTIONS_ADVANCED_NONRUNTIMEDESCRIPTION = (
        "server.modelConfig.sections.advanced.nonRuntimeDescription"
    )
    SERVER_MODELCONFIG_SECTIONS_INFERENCE_DESCRIPTION = (
        "server.modelConfig.sections.inference.description"
    )
    SERVER_MODELCONFIG_SECTIONS_INFERENCE_LABEL = (
        "server.modelConfig.sections.inference.label"
    )
    SERVER_MODELCONFIG_SECTIONS_LOAD_DESCRIPTION = (
        "server.modelConfig.sections.load.description"
    )
    SERVER_MODELCONFIG_SECTIONS_LOAD_LABEL = "server.modelConfig.sections.load.label"
    SERVER_MODELCONFIG_SECTIONS_LOAD_NONRUNTIMEDESCRIPTION = (
        "server.modelConfig.sections.load.nonRuntimeDescription"
    )
    SERVER_MODELCONFIG_SECTIONS_SOURCE_DESCRIPTION = (
        "server.modelConfig.sections.source.description"
    )
    SERVER_MODELCONFIG_SECTIONS_SOURCE_LABEL = (
        "server.modelConfig.sections.source.label"
    )
    SERVER_MODELCONFIG_SECTIONS_SUMMARY_DESCRIPTION = (
        "server.modelConfig.sections.summary.description"
    )
    SERVER_MODELCONFIG_SECTIONS_SUMMARY_LABEL = (
        "server.modelConfig.sections.summary.label"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_ADMINTOKEN = (
        "server.settings.properties.description.adminToken"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_AGENTDEBUGTRACE = (
        "server.settings.properties.description.agentDebugTrace"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_AGENTMEMORIES = (
        "server.settings.properties.description.agentMemories"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_AGENTMEMORYMODEL = (
        "server.settings.properties.description.agentMemoryModel"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_ALLOWEDORIGINS = (
        "server.settings.properties.description.allowedOrigins"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_ASSISTANTERRORENVELOPERENDERING = (
        "server.settings.properties.description.assistantErrorEnvelopeRendering"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_ASSISTANTSSERESUME = (
        "server.settings.properties.description.assistantSseResume"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_AUTOUNLOADENABLED = (
        "server.settings.properties.description.autoUnloadEnabled"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_AUTOUNLOADIDLEMINUTES = (
        "server.settings.properties.description.autoUnloadIdleMinutes"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_AUTOUNLOADMAXPRESSUREEVICTIONSPERLOAD = (
        "server.settings.properties.description.autoUnloadMaxPressureEvictionsPerLoad"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_AUTOUNLOADMINFREEGPUMEMORYBYTES = (
        "server.settings.properties.description.autoUnloadMinFreeGpuMemoryBytes"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_AUTOUNLOADMINFREESYSTEMMEMORYBYTES = (
        "server.settings.properties.description.autoUnloadMinFreeSystemMemoryBytes"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_CAPTUREGENAICONTENT = (
        "server.settings.properties.description.captureGenaiContent"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_CLOUDHTTPTRACE = (
        "server.settings.properties.description.cloudHttpTrace"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_DATABASEURL = (
        "server.settings.properties.description.databaseUrl"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_DEFAULTPROVIDER = (
        "server.settings.properties.description.defaultProvider"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_ENVIRONMENT = (
        "server.settings.properties.description.environment"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_EXTENSIONRETENTIONDAYS = (
        "server.settings.properties.description.extensionRetentionDays"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_EXTERNALHOOKS = (
        "server.settings.properties.description.externalHooks"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_FFMPEGAUTODOWNLOAD = (
        "server.settings.properties.description.ffmpegAutoDownload"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_FFMPEGENABLED = (
        "server.settings.properties.description.ffmpegEnabled"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_FFMPEGINSTALLDIR = (
        "server.settings.properties.description.ffmpegInstallDir"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICADDRESS = (
        "server.settings.properties.description.genericAddress"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICARTIFACT = (
        "server.settings.properties.description.genericArtifact"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICCONCURRENTREQUESTS = (
        "server.settings.properties.description.genericConcurrentRequests"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICCONTEXTLENGTH = (
        "server.settings.properties.description.genericContextLength"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICENABLED = (
        "server.settings.properties.description.genericEnabled"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICFLASHATTENTION = (
        "server.settings.properties.description.genericFlashAttention"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICINSTALLDIRECTORY = (
        "server.settings.properties.description.genericInstallDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICIPCPATH = (
        "server.settings.properties.description.genericIpcPath"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICJSONLOGS = (
        "server.settings.properties.description.genericJsonLogs"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICLOGLEVEL = (
        "server.settings.properties.description.genericLogLevel"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICPATH = (
        "server.settings.properties.description.genericPath"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICQUEUE = (
        "server.settings.properties.description.genericQueue"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_GENERICVERSION = (
        "server.settings.properties.description.genericVersion"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_INTERFACELANGUAGE = (
        "server.settings.properties.description.interfaceLanguage"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_JSONLOGS = (
        "server.settings.properties.description.jsonLogs"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_JSRUNTIMETRANSPORT = (
        "server.settings.properties.description.jsRuntimeTransport"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_LEGACYHOOKSCRIPTS = (
        "server.settings.properties.description.legacyHookScripts"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_LOGDIRECTORY = (
        "server.settings.properties.description.logDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_LOGLEVEL = (
        "server.settings.properties.description.logLevel"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_MAXUNUSEDDAYS = (
        "server.settings.properties.description.maxUnusedDays"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_MCPSERVERS = (
        "server.settings.properties.description.mcpServers"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_MCPTOOLS = (
        "server.settings.properties.description.mcpTools"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_MEMORYROOT = (
        "server.settings.properties.description.memoryRoot"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_METRICSEXPORTER = (
        "server.settings.properties.description.metricsExporter"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_MODELCACHEDIRECTORY = (
        "server.settings.properties.description.modelCacheDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_MODELCONFIGDIRECTORY = (
        "server.settings.properties.description.modelConfigDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_MODELSOURCE = (
        "server.settings.properties.description.modelSource"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PHASE1CONCURRENCY = (
        "server.settings.properties.description.phase1Concurrency"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PHASE1IDLESECONDS = (
        "server.settings.properties.description.phase1IdleSeconds"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PHASE1LEASESECONDS = (
        "server.settings.properties.description.phase1LeaseSeconds"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PHASE1MAXAGEDAYS = (
        "server.settings.properties.description.phase1MaxAgeDays"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PHASE1RETRYSECONDS = (
        "server.settings.properties.description.phase1RetrySeconds"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PHASE1SCANLIMIT = (
        "server.settings.properties.description.phase1ScanLimit"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PHASE2LEASESECONDS = (
        "server.settings.properties.description.phase2LeaseSeconds"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PHASE2LIMIT = (
        "server.settings.properties.description.phase2Limit"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PLUGININSTALLDIRECTORY = (
        "server.settings.properties.description.pluginInstallDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PROVIDERREGISTRY = (
        "server.settings.properties.description.providerRegistry"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_PYTHONRUNTIMETRANSPORT = (
        "server.settings.properties.description.pythonRuntimeTransport"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_RUNTIMEMODE = (
        "server.settings.properties.description.runtimeMode"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_RUNTIMETRANSPORT = (
        "server.settings.properties.description.runtimeTransport"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_SERVERADDRESS = (
        "server.settings.properties.description.serverAddress"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_SERVICENAME = (
        "server.settings.properties.description.serviceName"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_SERVICEVERSION = (
        "server.settings.properties.description.serviceVersion"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_SESSIONSTATEDIRECTORY = (
        "server.settings.properties.description.sessionStateDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_SPANATTRIBUTES = (
        "server.settings.properties.description.spanAttributes"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_SWAGGERENABLED = (
        "server.settings.properties.description.swaggerEnabled"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_TELEMETRY = (
        "server.settings.properties.description.telemetry"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_TRACESTATE = (
        "server.settings.properties.description.traceState"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_WEBSEARCHPROVIDERS = (
        "server.settings.properties.description.webSearchProviders"
    )
    SERVER_SETTINGS_PROPERTIES_DESCRIPTION_WORKSPACEMONACOLAZY = (
        "server.settings.properties.description.workspaceMonacoLazy"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_ADMINTOKEN = (
        "server.settings.properties.label.adminToken"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_AGENTDEBUGTRACE = (
        "server.settings.properties.label.agentDebugTrace"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_AGENTMEMORIES = (
        "server.settings.properties.label.agentMemories"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_AGENTMEMORYMODEL = (
        "server.settings.properties.label.agentMemoryModel"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_ALLOWEDORIGINS = (
        "server.settings.properties.label.allowedOrigins"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_ASSISTANTERRORENVELOPERENDERING = (
        "server.settings.properties.label.assistantErrorEnvelopeRendering"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_ASSISTANTSSERESUME = (
        "server.settings.properties.label.assistantSseResume"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_AUTOUNLOADIDLEMINUTES = (
        "server.settings.properties.label.autoUnloadIdleMinutes"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_AUTOUNLOADMAXPRESSUREEVICTIONSPERLOAD = (
        "server.settings.properties.label.autoUnloadMaxPressureEvictionsPerLoad"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_AUTOUNLOADMINFREEGPUMEMORYBYTES = (
        "server.settings.properties.label.autoUnloadMinFreeGpuMemoryBytes"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_AUTOUNLOADMINFREESYSTEMMEMORYBYTES = (
        "server.settings.properties.label.autoUnloadMinFreeSystemMemoryBytes"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_BINDADDRESS = (
        "server.settings.properties.label.bindAddress"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_CAPTUREGENAICONTENT = (
        "server.settings.properties.label.captureGenaiContent"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_CLOUDHTTPTRACE = (
        "server.settings.properties.label.cloudHttpTrace"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_DATABASEURL = (
        "server.settings.properties.label.databaseUrl"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_DEFAULTPROVIDER = (
        "server.settings.properties.label.defaultProvider"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_ENVIRONMENT = (
        "server.settings.properties.label.environment"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_EXTENSIONRETENTIONDAYS = (
        "server.settings.properties.label.extensionRetentionDays"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_EXTERNALHOOKS = (
        "server.settings.properties.label.externalHooks"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_FLASHATTENTION = (
        "server.settings.properties.label.flashAttention"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICADDRESS = (
        "server.settings.properties.label.genericAddress"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICARTIFACT = (
        "server.settings.properties.label.genericArtifact"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICAUTODOWNLOAD = (
        "server.settings.properties.label.genericAutoDownload"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICCONCURRENTREQUESTS = (
        "server.settings.properties.label.genericConcurrentRequests"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICCONTEXTLENGTH = (
        "server.settings.properties.label.genericContextLength"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICENABLED = (
        "server.settings.properties.label.genericEnabled"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICINSTALLDIRECTORY = (
        "server.settings.properties.label.genericInstallDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICIPCPATH = (
        "server.settings.properties.label.genericIpcPath"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICJSONLOGS = (
        "server.settings.properties.label.genericJsonLogs"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICLOGLEVEL = (
        "server.settings.properties.label.genericLogLevel"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICPATH = (
        "server.settings.properties.label.genericPath"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICQUEUE = (
        "server.settings.properties.label.genericQueue"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_GENERICVERSION = (
        "server.settings.properties.label.genericVersion"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_INTERFACELANGUAGE = (
        "server.settings.properties.label.interfaceLanguage"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_JSONLOGS = (
        "server.settings.properties.label.jsonLogs"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_JSRUNTIMETRANSPORT = (
        "server.settings.properties.label.jsRuntimeTransport"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_LEGACYHOOKSCRIPTS = (
        "server.settings.properties.label.legacyHookScripts"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_LOGDIRECTORY = (
        "server.settings.properties.label.logDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_LOGLEVEL = (
        "server.settings.properties.label.logLevel"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_MAXUNUSEDDAYS = (
        "server.settings.properties.label.maxUnusedDays"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_MCPSERVERS = (
        "server.settings.properties.label.mcpServers"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_MCPTOOLS = (
        "server.settings.properties.label.mcpTools"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_MEMORYROOT = (
        "server.settings.properties.label.memoryRoot"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_METRICSEXPORTER = (
        "server.settings.properties.label.metricsExporter"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_MODELCACHEDIRECTORY = (
        "server.settings.properties.label.modelCacheDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_MODELCONFIGDIRECTORY = (
        "server.settings.properties.label.modelConfigDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_MODELSOURCE = (
        "server.settings.properties.label.modelSource"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PHASE1CONCURRENCY = (
        "server.settings.properties.label.phase1Concurrency"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PHASE1IDLESECONDS = (
        "server.settings.properties.label.phase1IdleSeconds"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PHASE1LEASESECONDS = (
        "server.settings.properties.label.phase1LeaseSeconds"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PHASE1MAXAGEDAYS = (
        "server.settings.properties.label.phase1MaxAgeDays"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PHASE1RETRYSECONDS = (
        "server.settings.properties.label.phase1RetrySeconds"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PHASE1SCANLIMIT = (
        "server.settings.properties.label.phase1ScanLimit"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PHASE2LEASESECONDS = (
        "server.settings.properties.label.phase2LeaseSeconds"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PHASE2LIMIT = (
        "server.settings.properties.label.phase2Limit"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PLUGININSTALLDIRECTORY = (
        "server.settings.properties.label.pluginInstallDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PROVIDERREGISTRY = (
        "server.settings.properties.label.providerRegistry"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_PYTHONRUNTIMETRANSPORT = (
        "server.settings.properties.label.pythonRuntimeTransport"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_RUNTIMEMODE = (
        "server.settings.properties.label.runtimeMode"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_SERVICENAME = (
        "server.settings.properties.label.serviceName"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_SERVICEVERSION = (
        "server.settings.properties.label.serviceVersion"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_SESSIONSTATEDIRECTORY = (
        "server.settings.properties.label.sessionStateDirectory"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_SPANATTRIBUTES = (
        "server.settings.properties.label.spanAttributes"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_TELEMETRY = (
        "server.settings.properties.label.telemetry"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_TRACESTATE = (
        "server.settings.properties.label.traceState"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_TRANSPORT = (
        "server.settings.properties.label.transport"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_WEBSEARCHPROVIDERS = (
        "server.settings.properties.label.webSearchProviders"
    )
    SERVER_SETTINGS_PROPERTIES_LABEL_WORKSPACEMONACOLAZY = (
        "server.settings.properties.label.workspaceMonacoLazy"
    )
    SERVER_SETTINGS_SCHEMAS_MCP_ARGS_TITLE = "server.settings.schemas.mcp.args.title"
    SERVER_SETTINGS_SCHEMAS_MCP_COMMAND_DESCRIPTION = (
        "server.settings.schemas.mcp.command.description"
    )
    SERVER_SETTINGS_SCHEMAS_MCP_COMMAND_TITLE = (
        "server.settings.schemas.mcp.command.title"
    )
    SERVER_SETTINGS_SCHEMAS_MCP_CWD_TITLE = "server.settings.schemas.mcp.cwd.title"
    SERVER_SETTINGS_SCHEMAS_MCP_ENABLED_TITLE = (
        "server.settings.schemas.mcp.enabled.title"
    )
    SERVER_SETTINGS_SCHEMAS_MCP_ENVREFERENCE_TITLE = (
        "server.settings.schemas.mcp.envReference.title"
    )
    SERVER_SETTINGS_SCHEMAS_MCP_ENVVAR_TITLE = (
        "server.settings.schemas.mcp.envVar.title"
    )
    SERVER_SETTINGS_SCHEMAS_MCP_ENV_DESCRIPTION = (
        "server.settings.schemas.mcp.env.description"
    )
    SERVER_SETTINGS_SCHEMAS_MCP_ENV_TITLE = "server.settings.schemas.mcp.env.title"
    SERVER_SETTINGS_SCHEMAS_MCP_NAME_DESCRIPTION = (
        "server.settings.schemas.mcp.name.description"
    )
    SERVER_SETTINGS_SCHEMAS_MCP_NAME_TITLE = "server.settings.schemas.mcp.name.title"
    SERVER_SETTINGS_SCHEMAS_MCP_SERVER_TITLE = (
        "server.settings.schemas.mcp.server.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_APIBASE_TITLE = (
        "server.settings.schemas.provider.apiBase.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_APIKEYENV_TITLE = (
        "server.settings.schemas.provider.apiKeyEnv.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_APIKEY_TITLE = (
        "server.settings.schemas.provider.apiKey.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_AUTH_TITLE = (
        "server.settings.schemas.provider.auth.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_DISPLAYNAME_TITLE = (
        "server.settings.schemas.provider.displayName.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_ENTRY_TITLE = (
        "server.settings.schemas.provider.entry.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_FAMILY_TITLE = (
        "server.settings.schemas.provider.family.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_HEADERS_TITLE = (
        "server.settings.schemas.provider.headers.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_ID_DESCRIPTION = (
        "server.settings.schemas.provider.id.description"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_ID_TITLE = (
        "server.settings.schemas.provider.id.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_QUERY_TITLE = (
        "server.settings.schemas.provider.query.title"
    )
    SERVER_SETTINGS_SCHEMAS_PROVIDER_REQUESTDEFAULTS_TITLE = (
        "server.settings.schemas.provider.requestDefaults.title"
    )
    SERVER_SETTINGS_SCHEMAS_STRINGENTRY_TITLE = (
        "server.settings.schemas.stringEntry.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_BASEURL_TITLE = (
        "server.settings.schemas.webSearch.baseUrl.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_ENGINE_TITLE = (
        "server.settings.schemas.webSearch.engine.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_INCLUDEANSWER_TITLE = (
        "server.settings.schemas.webSearch.includeAnswer.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_INCLUDECONTENTS_TITLE = (
        "server.settings.schemas.webSearch.includeContents.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_INCLUDEIMAGES_TITLE = (
        "server.settings.schemas.webSearch.includeImages.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_INCLUDERAWCONTENT_TITLE = (
        "server.settings.schemas.webSearch.includeRawContent.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_MODEL_TITLE = (
        "server.settings.schemas.webSearch.model.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_SEARCHDEPTH_TITLE = (
        "server.settings.schemas.webSearch.searchDepth.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_SEARCHENGINEID_TITLE = (
        "server.settings.schemas.webSearch.searchEngineId.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_USELITE_TITLE = (
        "server.settings.schemas.webSearch.useLite.title"
    )
    SERVER_SETTINGS_SCHEMAS_WEBSEARCH_USERAGENT_TITLE = (
        "server.settings.schemas.webSearch.userAgent.title"
    )
    SERVER_SETTINGS_SECTIONS_AGENT_DESCRIPTION = (
        "server.settings.sections.agent.description"
    )
    SERVER_SETTINGS_SECTIONS_AGENT_TITLE = "server.settings.sections.agent.title"
    SERVER_SETTINGS_SECTIONS_DATABASE_DESCRIPTION = (
        "server.settings.sections.database.description"
    )
    SERVER_SETTINGS_SECTIONS_DATABASE_TITLE = "server.settings.sections.database.title"
    SERVER_SETTINGS_SECTIONS_GENERAL_DESCRIPTION = (
        "server.settings.sections.general.description"
    )
    SERVER_SETTINGS_SECTIONS_GENERAL_TITLE = "server.settings.sections.general.title"
    SERVER_SETTINGS_SECTIONS_GUARDRAILS_DESCRIPTION = (
        "server.settings.sections.guardrails.description"
    )
    SERVER_SETTINGS_SECTIONS_GUARDRAILS_TITLE = (
        "server.settings.sections.guardrails.title"
    )
    SERVER_SETTINGS_SECTIONS_LOGGING_DESCRIPTION = (
        "server.settings.sections.logging.description"
    )
    SERVER_SETTINGS_SECTIONS_LOGGING_TITLE = "server.settings.sections.logging.title"
    SERVER_SETTINGS_SECTIONS_MODELS_DESCRIPTION = (
        "server.settings.sections.models.description"
    )
    SERVER_SETTINGS_SECTIONS_MODELS_TITLE = "server.settings.sections.models.title"
    SERVER_SETTINGS_SECTIONS_PLUGIN_DESCRIPTION = (
        "server.settings.sections.plugin.description"
    )
    SERVER_SETTINGS_SECTIONS_PLUGIN_TITLE = "server.settings.sections.plugin.title"
    SERVER_SETTINGS_SECTIONS_PROVIDERS_DESCRIPTION = (
        "server.settings.sections.providers.description"
    )
    SERVER_SETTINGS_SECTIONS_PROVIDERS_TITLE = (
        "server.settings.sections.providers.title"
    )
    SERVER_SETTINGS_SECTIONS_RUNTIME_DESCRIPTION = (
        "server.settings.sections.runtime.description"
    )
    SERVER_SETTINGS_SECTIONS_RUNTIME_TITLE = "server.settings.sections.runtime.title"
    SERVER_SETTINGS_SECTIONS_SERVER_DESCRIPTION = (
        "server.settings.sections.server.description"
    )
    SERVER_SETTINGS_SECTIONS_SERVER_TITLE = "server.settings.sections.server.title"
    SERVER_SETTINGS_SECTIONS_TELEMETRY_DESCRIPTION = (
        "server.settings.sections.telemetry.description"
    )
    SERVER_SETTINGS_SECTIONS_TELEMETRY_TITLE = (
        "server.settings.sections.telemetry.title"
    )
    SERVER_SETTINGS_SECTIONS_TOOLS_DESCRIPTION = (
        "server.settings.sections.tools.description"
    )
    SERVER_SETTINGS_SECTIONS_TOOLS_TITLE = "server.settings.sections.tools.title"
    SERVER_SETTINGS_SUBSECTIONS_AGENT_GENERAL_DESCRIPTION = (
        "server.settings.subsections.agent.general.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_AGENT_HOOKS_DESCRIPTION = (
        "server.settings.subsections.agent.hooks.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_AGENT_HOOKS_TITLE = (
        "server.settings.subsections.agent.hooks.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_AGENT_MCP_DESCRIPTION = (
        "server.settings.subsections.agent.mcp.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_AGENT_MCP_TITLE = (
        "server.settings.subsections.agent.mcp.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_AGENT_MEMORIES_DESCRIPTION = (
        "server.settings.subsections.agent.memories.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_AGENT_MEMORIES_TITLE = (
        "server.settings.subsections.agent.memories.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_AGENT_WEBSEARCH_DESCRIPTION = (
        "server.settings.subsections.agent.websearch.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_AGENT_WEBSEARCH_TITLE = (
        "server.settings.subsections.agent.websearch.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_DATABASE_GENERAL_DESCRIPTION = (
        "server.settings.subsections.database.general.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_GENERAL_GENERAL_DESCRIPTION = (
        "server.settings.subsections.general.general.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_GENERAL_GENERAL_TITLE = (
        "server.settings.subsections.general.general.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_GUARDRAILS_ROLLBACK_DESCRIPTION = (
        "server.settings.subsections.guardrails.rollback.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_GUARDRAILS_ROLLBACK_TITLE = (
        "server.settings.subsections.guardrails.rollback.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_LOGGING_GENERAL_DESCRIPTION = (
        "server.settings.subsections.logging.general.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_MODELS_AUTOUNLOAD_DESCRIPTION = (
        "server.settings.subsections.models.autoUnload.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_MODELS_AUTOUNLOAD_TITLE = (
        "server.settings.subsections.models.autoUnload.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_MODELS_GENERAL_DESCRIPTION = (
        "server.settings.subsections.models.general.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_PLUGIN_GENERAL_DESCRIPTION = (
        "server.settings.subsections.plugin.general.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_PROVIDERS_REGISTRY_DESCRIPTION = (
        "server.settings.subsections.providers.registry.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_PROVIDERS_REGISTRY_TITLE = (
        "server.settings.subsections.providers.registry.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_CANDLE_DESCRIPTION = (
        "server.settings.subsections.runtime.candle.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_CANDLE_TITLE = (
        "server.settings.subsections.runtime.candle.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_DIFFUSION_DESCRIPTION = (
        "server.settings.subsections.runtime.diffusion.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_DIFFUSION_TITLE = (
        "server.settings.subsections.runtime.diffusion.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_GENERAL_DESCRIPTION = (
        "server.settings.subsections.runtime.general.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_GGML_DESCRIPTION = (
        "server.settings.subsections.runtime.ggml.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_GGML_TITLE = (
        "server.settings.subsections.runtime.ggml.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_LLAMA_DESCRIPTION = (
        "server.settings.subsections.runtime.llama.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_LLAMA_TITLE = (
        "server.settings.subsections.runtime.llama.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_ONNX_DESCRIPTION = (
        "server.settings.subsections.runtime.onnx.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_ONNX_TITLE = (
        "server.settings.subsections.runtime.onnx.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_WHISPER_DESCRIPTION = (
        "server.settings.subsections.runtime.whisper.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_RUNTIME_WHISPER_TITLE = (
        "server.settings.subsections.runtime.whisper.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_SERVER_ADMIN_DESCRIPTION = (
        "server.settings.subsections.server.admin.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_SERVER_ADMIN_TITLE = (
        "server.settings.subsections.server.admin.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_SERVER_CORS_DESCRIPTION = (
        "server.settings.subsections.server.cors.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_SERVER_CORS_TITLE = (
        "server.settings.subsections.server.cors.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_SERVER_GENERAL_DESCRIPTION = (
        "server.settings.subsections.server.general.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_SERVER_SWAGGER_DESCRIPTION = (
        "server.settings.subsections.server.swagger.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_SERVER_SWAGGER_TITLE = (
        "server.settings.subsections.server.swagger.title"
    )
    SERVER_SETTINGS_SUBSECTIONS_TELEMETRY_GENERAL_DESCRIPTION = (
        "server.settings.subsections.telemetry.general.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_TOOLS_FFMPEG_DESCRIPTION = (
        "server.settings.subsections.tools.ffmpeg.description"
    )
    SERVER_SETTINGS_SUBSECTIONS_TOOLS_FFMPEG_TITLE = (
        "server.settings.subsections.tools.ffmpeg.title"
    )
    SERVER_TASKS_FFMPEG_AUDIOEXTRACTION = "server.tasks.ffmpeg.audioExtraction"
    SERVER_TASKS_FFMPEG_COMPLETED = "server.tasks.ffmpeg.completed"
    SERVER_TASKS_FFMPEG_CONVERSIONFAILED = "server.tasks.ffmpeg.conversionFailed"
    SERVER_TASKS_FFMPEG_REMUXCOMPLETED = "server.tasks.ffmpeg.remuxCompleted"
    SERVER_TASKS_FFMPEG_REMUXFAILED = "server.tasks.ffmpeg.remuxFailed"
    SERVER_TASKS_FFMPEG_RUNTIMEINITFAILED = "server.tasks.ffmpeg.runtimeInitFailed"
    SERVER_TASKS_FFMPEG_STARTING = "server.tasks.ffmpeg.starting"
    SERVER_TASKS_FFMPEG_UNSUPPORTEDOUTPUTFORMAT = (
        "server.tasks.ffmpeg.unsupportedOutputFormat"
    )
    SERVER_TASKS_FFMPEG_WORKERFAILED = "server.tasks.ffmpeg.workerFailed"
    SERVER_TASKS_SETUP_CHECKINGFFMPEG = "server.tasks.setup.checkingFfmpeg"
    SERVER_TASKS_SETUP_DOWNLOADEDPAYLOAD = "server.tasks.setup.downloadedPayload"
    SERVER_TASKS_SETUP_DOWNLOADINGPAYLOAD = "server.tasks.setup.downloadingPayload"
    SERVER_TASKS_SETUP_EXPANDINGRUNTIMEPAYLOAD = (
        "server.tasks.setup.expandingRuntimePayload"
    )
    SERVER_TASKS_SETUP_FAILEDBEFOREFINISH = "server.tasks.setup.failedBeforeFinish"
    SERVER_TASKS_SETUP_INSTALLINGRUNTIMELIBRARIES = (
        "server.tasks.setup.installingRuntimeLibraries"
    )
    SERVER_TASKS_SETUP_RESTARTINGRUNTIMEWORKERS = (
        "server.tasks.setup.restartingRuntimeWorkers"
    )
    SERVER_TASKS_SETUP_SELECTINGRUNTIMEPAYLOAD = (
        "server.tasks.setup.selectingRuntimePayload"
    )
    SERVER_TASKS_SETUP_USINGCACHEDPAYLOAD = "server.tasks.setup.usingCachedPayload"
    SERVER_TASKS_SETUP_USINGINSTALLEDRUNTIMEPAYLOAD = (
        "server.tasks.setup.usingInstalledRuntimePayload"
    )

    def __str__(self) -> str:
        return str(self.value)
