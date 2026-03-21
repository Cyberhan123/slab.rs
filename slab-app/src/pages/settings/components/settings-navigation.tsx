import { useEffect, useMemo, useState } from 'react';
import { ChevronRight } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible';
import { ScrollArea } from '@/components/ui/scroll-area';
import { SoftPanel, StatusPill } from '@/components/ui/workspace';
import { cn } from '@/lib/utils';

import type { SettingsSectionResponse } from '../types';
import {
  countSectionProperties,
  sectionAnchorId,
  shouldCollapseSubsectionHeading,
  subsectionAnchorId,
} from '../utils';

type SettingsNavigationProps = {
  activeTarget: string | null;
  dirtyCount: number;
  errorCount: number;
  savingCount: number;
  sections: SettingsSectionResponse[];
  onJump: (targetId: string) => void;
};

export function SettingsNavigation({
  activeTarget,
  dirtyCount,
  errorCount,
  savingCount,
  sections,
  onJump,
}: SettingsNavigationProps) {
  const [openSections, setOpenSections] = useState<Record<string, boolean>>({});

  const activeParentSectionId = useMemo(() => {
    if (!activeTarget) {
      return null;
    }

    for (const section of sections) {
      if (activeTarget === sectionAnchorId(section.id)) {
        return section.id;
      }

      if (
        section.subsections.some(
          (subsection) => subsectionAnchorId(section.id, subsection.id) === activeTarget,
        )
      ) {
        return section.id;
      }
    }

    return null;
  }, [activeTarget, sections]);

  useEffect(() => {
    if (!activeParentSectionId) {
      return;
    }

    setOpenSections((current) =>
      current[activeParentSectionId] ? current : { ...current, [activeParentSectionId]: true },
    );
  }, [activeParentSectionId]);

  function setSectionOpen(sectionId: string, isOpen: boolean) {
    setOpenSections((current) => ({ ...current, [sectionId]: isOpen }));
  }

  return (
    <SoftPanel className="space-y-4 rounded-[28px] border border-border/70 p-3">
      <div className="space-y-3 rounded-[22px] bg-[var(--surface-1)] px-4 py-4">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
            Navigation
          </p>
          <p className="mt-1 text-base font-semibold tracking-tight">Settings Outline</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <StatusPill status={errorCount > 0 ? 'danger' : 'success'}>
            {errorCount > 0 ? `${errorCount} issue${errorCount > 1 ? 's' : ''}` : 'No issues'}
          </StatusPill>
          <StatusPill status={savingCount > 0 ? 'info' : 'neutral'}>
            {savingCount > 0 ? `${savingCount} saving` : 'Saved'}
          </StatusPill>
          <Badge variant="counter">
            {dirtyCount > 0 ? `${dirtyCount} pending` : 'Synced'}
          </Badge>
        </div>
      </div>

      <ScrollArea className="max-h-[calc(100vh-18rem)] rounded-[20px] bg-[var(--surface-1)] p-2">
        <div className="flex flex-col gap-1.5">
          {sections.map((section) => {
            const sectionTargetId = sectionAnchorId(section.id);
            const visibleSubsections = section.subsections.filter(
              (subsection) => !shouldCollapseSubsectionHeading(section, subsection),
            );
            const hasActiveSubsection = section.subsections.some(
              (subsection) => subsectionAnchorId(section.id, subsection.id) === activeTarget,
            );
            const isSectionActive = activeTarget === sectionTargetId || hasActiveSubsection;
            const isOpen = openSections[section.id] || hasActiveSubsection;

            return (
              <Collapsible
                key={section.id}
                open={isOpen}
                onOpenChange={(nextOpen) => setSectionOpen(section.id, nextOpen)}
              >
                <CollapsibleTrigger asChild>
                  <Button
                    variant="quiet"
                    size="sm"
                    className={cn(
                      'group h-auto w-full justify-start gap-2 rounded-xl px-3 py-2 text-left',
                      isSectionActive &&
                        'bg-[var(--surface-selected)] text-foreground shadow-[inset_0_0_0_1px_color-mix(in_oklab,var(--border)_85%,white_15%)]',
                    )}
                    onClick={() => onJump(sectionTargetId)}
                  >
                    <ChevronRight
                      className={cn(
                        'h-4 w-4 shrink-0 text-muted-foreground transition-transform group-data-[state=open]:rotate-90',
                        isSectionActive && 'text-foreground/90',
                      )}
                    />
                    <div className="min-w-0 flex-1">
                      <p className="truncate font-medium">{section.title}</p>
                      <p className="text-xs text-muted-foreground">
                        {countSectionProperties(section)} settings
                      </p>
                    </div>
                  </Button>
                </CollapsibleTrigger>

                {visibleSubsections.length > 0 ? (
                  <CollapsibleContent className="overflow-hidden">
                    <div className="mt-1 ml-5 border-l border-border/60 pl-2">
                      {visibleSubsections.map((subsection) => {
                        const subsectionTargetId = subsectionAnchorId(section.id, subsection.id);

                        return (
                          <Button
                            key={subsection.id}
                            variant="quiet"
                            size="sm"
                            className={cn(
                              'h-auto w-full justify-start gap-2 rounded-xl px-3 py-1.5 text-left',
                              activeTarget === subsectionTargetId &&
                                'bg-[var(--surface-selected)] text-foreground',
                            )}
                            onClick={() => onJump(subsectionTargetId)}
                          >
                            <div className="min-w-0 flex-1">
                              <p className="truncate">{subsection.title}</p>
                              <p className="text-xs text-muted-foreground">
                                {subsection.properties.length} fields
                              </p>
                            </div>
                          </Button>
                        );
                      })}
                    </div>
                  </CollapsibleContent>
                ) : null}
              </Collapsible>
            );
          })}
        </div>
      </ScrollArea>
    </SoftPanel>
  );
}
