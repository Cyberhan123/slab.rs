import {
  Cloud,
  Cpu,
  Download,
  SlidersHorizontal,
  Sparkles,
  type LucideIcon,
} from 'lucide-react';

import { Badge } from '@slab/components/badge';
import { cn } from '@/lib/utils';

import type { SettingsSectionResponse } from '../types';
import { countSectionProperties } from '../utils';

type SettingsNavigationProps = {
  activeSectionId: string | null;
  sections: SettingsSectionResponse[];
  onSelectSection: (sectionId: string) => void;
};

const sectionIcons: Record<string, LucideIcon> = {
  cloud: Cloud,
  diffusion: Sparkles,
  runtime: Cpu,
  setup: Download,
};

export function SettingsNavigation({
  activeSectionId,
  sections,
  onSelectSection,
}: SettingsNavigationProps) {
  return (
    <div className="flex h-full flex-col px-4 py-5 lg:px-5 lg:py-6">
      <nav className="flex flex-col gap-1.5 overflow-y-auto pr-1">
        {sections.map((section) => {
          const SectionIcon = sectionIcons[section.id] ?? SlidersHorizontal;
          const isActiveSection = section.id === activeSectionId;

          return (
            <button
              key={section.id}
              type="button"
              onClick={() => onSelectSection(section.id)}
              className={cn(
                'flex w-full items-center gap-3 rounded-[16px] px-3 py-2.5 text-left transition-colors',
                isActiveSection
                  ? 'bg-[var(--shell-card)] text-[var(--brand-teal)] shadow-[var(--shell-elevation)]'
                  : 'text-muted-foreground hover:bg-[var(--shell-card)]/80 hover:text-foreground',
              )}
            >
              <span
                className={cn(
                  'flex size-8 shrink-0 items-center justify-center rounded-[12px]',
                  isActiveSection ? 'bg-[var(--brand-teal)]/10 text-[var(--brand-teal)]' : 'bg-transparent text-muted-foreground/70',
                )}
              >
                <SectionIcon className="size-4" />
              </span>

              <span className="min-w-0 flex-1">
                <span
                  className={cn(
                    'block truncate text-sm',
                    isActiveSection ? 'font-semibold text-[var(--brand-teal)]' : 'font-medium',
                  )}
                >
                  {section.title}
                </span>
              </span>

              {isActiveSection ? (
                <Badge
                  variant="counter"
                  className="rounded-full border-[var(--shell-card)]/80 bg-[var(--surface-soft)] px-2 py-0.5 text-[10px] font-bold text-muted-foreground shadow-none"
                >
                  {countSectionProperties(section)}
                </Badge>
              ) : null}
            </button>
          );
        })}
      </nav>
    </div>
  );
}
