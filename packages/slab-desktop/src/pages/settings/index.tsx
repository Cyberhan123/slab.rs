import { useEffect, useMemo, useRef, useState } from 'react';
import { useBlocker } from 'react-router-dom';
import { Loader2, RefreshCw, TriangleAlert } from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@slab/components/dialog';
import { StageEmptyState, StatusPill } from '@slab/components/workspace';
import { translateServerField, useTranslation } from '@slab/i18n';
import { usePageHeader, usePageHeaderSearch } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import { useUiStatePersistenceStatus } from '@/store/ui-state-storage';
import api, { getErrorMessage } from '@slab/api';

import { SettingFieldCard } from './components/setting-field-card';
import { SettingsNavigation } from './components/settings-navigation';
import { PluginPermissionsCard } from '../plugins/components/plugin-permissions-card';
import { useSettingsAutosave } from './hooks/use-settings-autosave';
import type { SettingResponse } from './types';
import {
  countSectionProperties,
  matchesSearch,
  sectionAnchorId,
  subsectionAnchorId,
  shouldCollapseSubsectionHeading,
} from './utils';

export default function SettingsPage() {
  const canvasRef = useRef<HTMLDivElement | null>(null);
  const [activeSectionId, setActiveSectionId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const { t } = useTranslation();
  const uiStateFailure = useUiStatePersistenceStatus((state) => state.lastFailure);

  const { data, error, isLoading, refetch } = api.useQuery('get', '/v1/settings');

  const propertyMap = useMemo(() => {
    const map = new Map<string, SettingResponse>();

    for (const section of data?.sections ?? []) {
      for (const subsection of section.subsections) {
        for (const property of subsection.properties) {
          map.set(property.pmid, property);
        }
      }
    }

    return map;
  }, [data]);

  const sections = useMemo(() => data?.sections ?? [], [data?.sections]);
  const normalizedSearchQuery = searchQuery.trim().toLowerCase();
  const visibleSections = useMemo(() => {
    if (!normalizedSearchQuery) {
      return sections;
    }

    return sections
      .map((section) => ({
        ...section,
        subsections: section.subsections
          .map((subsection) => ({
            ...subsection,
            properties: subsection.properties.filter((property) =>
              matchesSearch(section, subsection, property, normalizedSearchQuery),
            ),
          }))
          .filter((subsection) => subsection.properties.length > 0),
      }))
      .filter((section) => section.subsections.length > 0);
  }, [normalizedSearchQuery, sections]);
  const shouldShowAdminTokenWarning = useMemo(
    () => shouldWarnForMissingAdminToken(propertyMap),
    [propertyMap],
  );
  const activeSection = useMemo(
    () => visibleSections.find((section) => section.id === activeSectionId) ?? visibleSections[0] ?? null,
    [activeSectionId, visibleSections],
  );
  const activeSectionTitle = activeSection
    ? translateServerField(activeSection.i18n, 'title', activeSection.title, t)
    : '';
  const activeSectionDescription = activeSection
    ? translateServerField(activeSection.i18n, 'description_md', activeSection.description_md, t)
    : '';

  useEffect(() => {
    if (visibleSections.length === 0) {
      setActiveSectionId(null);
      return;
    }

    if (activeSectionId && visibleSections.some((section) => section.id === activeSectionId)) {
      return;
    }

    const nextSectionId = visibleSections[0].id;
    setActiveSectionId(nextSectionId);
  }, [activeSectionId, visibleSections]);

  usePageHeader({
    ...PAGE_HEADER_META.settings,
    title: t('pages.settings.header.title'),
    subtitle: t('pages.settings.header.subtitle'),
  });
  usePageHeaderSearch({
    type: 'search',
    value: searchQuery,
    onValueChange: setSearchQuery,
    placeholder: t('pages.settings.search.placeholder'),
    ariaLabel: t('pages.settings.search.ariaLabel'),
  });

  const {
    drafts,
    fieldErrors,
    fieldStatuses,
    resettingPmid,
    statusSummary,
    setDraftValue,
    resetSetting,
  } = useSettingsAutosave({
    propertyMap,
    refetch,
  });
  const unsavedSettingsCount = statusSummary.dirty + statusSummary.saving + statusSummary.error;
  const hasUnsavedSettings = unsavedSettingsCount > 0;
  const blocker = useBlocker(({ currentLocation, nextLocation }) =>
    hasUnsavedSettings && currentLocation.pathname !== nextLocation.pathname,
  );

  useEffect(() => {
    if (!hasUnsavedSettings) {
      return undefined;
    }

    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = '';
    };

    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => {
      window.removeEventListener('beforeunload', handleBeforeUnload);
    };
  }, [hasUnsavedSettings]);

  function selectSection(sectionId: string) {
    const targetId = sectionAnchorId(sectionId);
    setActiveSectionId(sectionId);
    canvasRef.current?.scrollTo({ top: 0, behavior: 'smooth' });
    scrollToTarget(targetId);
  }

  if (isLoading) {
    return (
      <StageEmptyState
        icon={Loader2}
        title={t('pages.settings.page.loadingTitle')}
        description={t('pages.settings.page.loadingDescription')}
        className="[&_svg]:animate-spin"
      />
    );
  }

  if (!data) {
    return (
      <div className="mx-auto flex max-w-3xl flex-col gap-4 py-10">
        <Alert variant="destructive">
          <TriangleAlert className="h-4 w-4" />
          <AlertTitle>{t('pages.settings.page.failedLoadTitle')}</AlertTitle>
          <AlertDescription>
            {getErrorMessage(error ?? new Error('Unknown settings error.'))}
          </AlertDescription>
        </Alert>
        <div>
          <Button variant="pill" size="pill" onClick={() => void refetch()}>
            <RefreshCw className="mr-2 h-4 w-4" />
            {t('pages.settings.page.tryAgain')}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col overflow-hidden rounded-2xl border border-border/50 bg-[var(--shell-card)] shadow-[var(--shell-elevation)] lg:flex-row">
      <aside className="w-full shrink-0 border-b border-border/50 bg-[color:color-mix(in_oklab,var(--surface-soft)_80%,transparent)] lg:w-[256px] lg:border-r lg:border-b-0">
          <SettingsNavigation
            activeSectionId={activeSection?.id ?? null}
          sections={visibleSections}
          onSelectSection={selectSection}
        />
      </aside>

      <div ref={canvasRef} className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex w-full max-w-[944px] flex-col gap-6 px-6 py-6 md:px-8 md:py-8">
          {data.warnings.length > 0 ? (
            <Alert>
              <TriangleAlert className="h-4 w-4" />
              <AlertTitle>{t('pages.settings.page.warningsTitle')}</AlertTitle>
              <AlertDescription>
                <div className="space-y-1">
                  {data.warnings.map((warning) => (
                    <p key={warning}>{warning}</p>
                  ))}
                </div>
              </AlertDescription>
            </Alert>
          ) : null}

          {shouldShowAdminTokenWarning ? (
            <Alert variant="destructive">
              <TriangleAlert className="h-4 w-4" />
              <AlertTitle>{t('pages.settings.page.adminTokenWarningTitle')}</AlertTitle>
              <AlertDescription>
                {t('pages.settings.page.adminTokenWarningDescription')}
              </AlertDescription>
            </Alert>
          ) : null}

          {uiStateFailure ? (
            <Alert variant="destructive">
              <TriangleAlert className="h-4 w-4" />
              <AlertTitle>{t('pages.settings.persistence.warningTitle')}</AlertTitle>
              <AlertDescription>
                {t('pages.settings.persistence.warningDescription', {
                  key: uiStateFailure.key,
                  message: uiStateFailure.message,
                  operation: t(`pages.settings.persistence.operations.${uiStateFailure.operation}`),
                })}
              </AlertDescription>
            </Alert>
          ) : null}

          <PluginPermissionsCard />

          {!activeSection ? (
            <StageEmptyState
              title={
                normalizedSearchQuery
                  ? t('pages.settings.search.noResultsTitle')
                  : t('pages.settings.page.noSettingsTitle')
              }
              description={
                normalizedSearchQuery
                  ? t('pages.settings.search.noResultsDescription', { query: searchQuery.trim() })
                  : t('pages.settings.page.noSettingsDescription')
              }
            />
          ) : (
            <>
              <header
                id={sectionAnchorId(activeSection.id)}
                className="scroll-mt-6 space-y-4 border-b border-border/50 pb-6"
              >
                <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                  <div className="space-y-3">
                    <div className="flex flex-wrap items-center gap-3">
                      <h1 className="text-3xl font-bold tracking-display text-foreground">
                        {activeSectionTitle}
                      </h1>
                      <Badge
                        variant="chip"
                        className="rounded-full border-border/60 bg-border/30 px-3 py-1 text-micro font-bold uppercase tracking-eyebrow text-muted-foreground"
                      >
                        {t('pages.settings.page.settingsCount', {
                          count: countSectionProperties(activeSection),
                        })}
                      </Badge>
                    </div>
                    {activeSectionDescription ? (
                      <p className="max-w-3xl text-base leading-8 text-muted-foreground">
                        {activeSectionDescription}
                      </p>
                    ) : null}
                    <p className="max-w-3xl truncate font-mono text-caption text-muted-foreground/80">
                      {data.settings_path}
                    </p>
                  </div>

                  {statusSummary.error > 0 ||
                  statusSummary.saving > 0 ||
                  statusSummary.dirty > 0 ? (
                    <div className="flex flex-wrap items-center gap-2">
                      {statusSummary.error > 0 ? (
                        <StatusPill status="danger">
                          {t('pages.settings.page.issues', { count: statusSummary.error })}
                        </StatusPill>
                      ) : null}
                      {statusSummary.saving > 0 ? (
                        <StatusPill status="info">
                          {t('pages.settings.page.saving', { count: statusSummary.saving })}
                        </StatusPill>
                      ) : null}
                      {statusSummary.dirty > 0 ? (
                        <Badge variant="counter">
                          {t('pages.settings.page.pending', { count: statusSummary.dirty })}
                        </Badge>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              </header>

              <div className="space-y-6 pb-8">
                {activeSection.subsections.map((subsection) => {
                  const subsectionTitle = translateServerField(
                    subsection.i18n,
                    'title',
                    subsection.title,
                    t,
                  );
                  const subsectionDescription = translateServerField(
                    subsection.i18n,
                    'description_md',
                    subsection.description_md,
                    t,
                  );
                  return (
                    <section
                      key={subsection.id}
                      id={subsectionAnchorId(activeSection.id, subsection.id)}
                      className="scroll-mt-8 rounded-[20px] border border-border/40 bg-[color:color-mix(in_oklab,var(--surface-soft)_70%,transparent)] p-6 md:p-8"
                    >
                      {shouldCollapseSubsectionHeading(activeSection, subsection) ? (
                        subsectionDescription ? (
                          <p className="text-sm leading-7 text-muted-foreground">
                            {subsectionDescription}
                          </p>
                        ) : null
                      ) : (
                        <div className="space-y-2">
                          <div className="flex flex-wrap items-center gap-3">
                            <h2 className="text-lg font-bold tracking-tight text-foreground">
                              {subsectionTitle}
                            </h2>
                          </div>
                          {subsectionDescription ? (
                            <p className="text-sm leading-7 text-muted-foreground">
                              {subsectionDescription}
                            </p>
                          ) : null}
                        </div>
                      )}

                      <div className="mt-6 grid gap-4 xl:grid-cols-2">
                        {subsection.properties.map((property) => (
                          <div
                            key={property.pmid}
                            className={shouldPropertySpanFullWidth(property) ? 'xl:col-span-2' : ''}
                          >
                            <SettingFieldCard
                              property={property}
                              draftValue={drafts[property.pmid]}
                              errorState={fieldErrors[property.pmid]}
                              fieldStatus={fieldStatuses[property.pmid]}
                              isResetting={resettingPmid === property.pmid}
                              onChange={setDraftValue}
                              onReset={resetSetting}
                            />
                          </div>
                        ))}
                      </div>
                    </section>
                  );
                })}
              </div>
            </>
          )}
        </div>
      </div>
      <Dialog
        open={blocker.state === 'blocked'}
        onOpenChange={(open) => {
          if (!open && blocker.state === 'blocked') {
            blocker.reset();
          }
        }}
      >
        <DialogContent className="max-w-md" data-testid="settings-unsaved-dialog">
          <DialogHeader>
            <DialogTitle>{t('pages.settings.guard.title')}</DialogTitle>
            <DialogDescription>
              {t('pages.settings.guard.description', { count: unsavedSettingsCount })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="quiet"
              onClick={() => {
                if (blocker.state === 'blocked') {
                  blocker.reset();
                }
              }}
              data-testid="settings-unsaved-cancel"
            >
              {t('pages.settings.guard.stay')}
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                if (blocker.state === 'blocked') {
                  blocker.proceed();
                }
              }}
              data-testid="settings-unsaved-leave"
            >
              {t('pages.settings.guard.leave')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function scrollToTarget(targetId: string) {
  window.requestAnimationFrame(() => {
    document.getElementById(targetId)?.scrollIntoView({
      behavior: 'smooth',
      block: 'start',
    });
  });
}

function shouldPropertySpanFullWidth(property: SettingResponse) {
  if (property.schema.multiline || property.schema.json_schema) {
    return true;
  }

  if (property.schema.type === 'array' || property.schema.type === 'object') {
    return true;
  }

  if (property.schema.type === 'tagged_union') {
    return true;
  }

  const label = property.label.toLowerCase();

  return (
    property.schema.type === 'boolean' ||
    label.includes('directory') ||
    label.includes('providers') ||
    label.includes('path')
  );
}

function shouldWarnForMissingAdminToken(propertyMap: Map<string, SettingResponse>) {
  const address = propertyMap.get('server.address')?.effective_value;
  const token = propertyMap.get('server.admin.token')?.effective_value;

  return typeof address === 'string' && !isLoopbackBindAddress(address) && !hasAdminToken(token);
}

function hasAdminToken(value: unknown) {
  return typeof value === 'string' && value.trim().length > 0;
}

function isLoopbackBindAddress(bindAddress: string) {
  const host = bindHost(bindAddress);
  return (
    host === 'localhost' ||
    host === '127.0.0.1' ||
    host === '::1' ||
    host.startsWith('127.')
  );
}

function bindHost(bindAddress: string) {
  const trimmed = bindAddress.trim();
  const bracketed = trimmed.match(/^\[([^\]]+)\](?::\d+)?$/);
  if (bracketed) {
    return bracketed[1].toLowerCase();
  }

  return trimmed.split(':')[0].toLowerCase();
}
