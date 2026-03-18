import { useEffect, useMemo, useState } from 'react';
import { ChevronRight } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible';
import { ScrollArea } from '@/components/ui/scroll-area';
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
    <Card className="overflow-hidden border-sidebar-border/70 bg-sidebar text-sidebar-foreground shadow-[0_20px_60px_-48px_color-mix(in_oklab,var(--foreground)_32%,transparent)]">
      <CardHeader className="gap-3 border-b border-sidebar-border/60 bg-sidebar/95">
        <CardTitle className="text-base">Settings outline</CardTitle>
        <div className="flex flex-wrap gap-2">
          <Badge
            variant={errorCount > 0 ? 'destructive' : 'outline'}
            className={errorCount === 0 ? 'border-sidebar-border text-sidebar-foreground' : ''}
          >
            {errorCount > 0 ? `${errorCount} issue${errorCount > 1 ? 's' : ''}` : 'No issues'}
          </Badge>
          <Badge
            variant={savingCount > 0 ? 'secondary' : 'outline'}
            className={
              savingCount > 0
                ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                : 'border-sidebar-border text-sidebar-foreground'
            }
          >
            {savingCount > 0 ? `${savingCount} saving` : 'Saved'}
          </Badge>
          <Badge
            variant={dirtyCount > 0 ? 'secondary' : 'outline'}
            className={
              dirtyCount > 0
                ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                : 'border-sidebar-border text-sidebar-foreground'
            }
          >
            {dirtyCount > 0 ? `${dirtyCount} pending` : 'Synced'}
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="p-0">
        <ScrollArea className="max-h-[calc(100vh-16rem)]">
          <div className="flex flex-col gap-1 p-3">
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
                  <div>
                    <CollapsibleTrigger asChild>
                      <Button
                        variant="ghost"
                        size="sm"
                        className={cn(
                          'group h-auto w-full justify-start gap-2 rounded-xl px-2 py-2 text-left text-sidebar-foreground transition-none hover:bg-sidebar-accent hover:text-sidebar-accent-foreground',
                          isSectionActive && 'bg-sidebar-accent text-sidebar-accent-foreground',
                        )}
                        onClick={() => onJump(sectionTargetId)}
                      >
                        <ChevronRight
                          className={cn(
                            'h-4 w-4 shrink-0 text-sidebar-foreground/60 transition-transform group-data-[state=open]:rotate-90',
                            isSectionActive && 'text-sidebar-accent-foreground/80',
                          )}
                        />
                        <div className="min-w-0 flex-1">
                          <p className="truncate font-medium">{section.title}</p>
                          <p className="text-xs text-sidebar-foreground/65">
                            {countSectionProperties(section)} settings
                          </p>
                        </div>
                      </Button>
                    </CollapsibleTrigger>

                    {visibleSubsections.length > 0 ? (
                      <CollapsibleContent className="overflow-hidden">
                        <div className="mt-1 ml-5 border-l border-sidebar-border/60 pl-2">
                          {visibleSubsections.map((subsection) => {
                            const subsectionTargetId = subsectionAnchorId(section.id, subsection.id);

                            return (
                              <Button
                                key={subsection.id}
                                variant="ghost"
                                size="sm"
                                className={cn(
                                  'h-auto w-full justify-start gap-2 rounded-xl px-2 py-1.5 text-left text-sidebar-foreground transition-none hover:bg-sidebar-accent hover:text-sidebar-accent-foreground',
                                  activeTarget === subsectionTargetId &&
                                    'bg-sidebar-accent text-sidebar-accent-foreground',
                                )}
                                onClick={() => onJump(subsectionTargetId)}
                              >
                                <div className="min-w-0 flex-1">
                                  <p className="truncate">{subsection.title}</p>
                                  <p className="text-xs text-sidebar-foreground/65">
                                    {subsection.properties.length} fields
                                  </p>
                                </div>
                              </Button>
                            );
                          })}
                        </div>
                      </CollapsibleContent>
                    ) : null}
                  </div>
                </Collapsible>
              );
            })}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  );
}
