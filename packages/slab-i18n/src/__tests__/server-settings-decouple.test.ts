import { describe, expect, it } from 'vitest';

// Import directly from the source module: `server.ts` only has a type-only
// import (`import type { components }`), so importing it triggers none of the
// i18next load-time side effects that `index.ts` would.
import { enUSServerMessages, zhCNServerMessages } from '../locales/server';

// Contract: the agent trace bundle (`agent.debug`) and the OpenTelemetry
// provider/export (`telemetry.enabled`) are INDEPENDENT diagnostic switches.
// Every user-facing description of one switch must cross-reference the other so
// the decoupling is visible in both locales. `locale-parity.test.ts` only checks
// key/placeholder shape, not semantic content — this test pins the content.
describe('settings descriptions keep agent.debug and telemetry.enabled decoupled', () => {
  type Messages = typeof enUSServerMessages;

  // Property-level descriptions must state independence AND cross-reference the
  // other switch in both locales.
  const propertyKeys = [
    'server.settings.properties.description.telemetry',
    'server.settings.properties.description.agentDebugTrace',
  ] as const;

  it.each(propertyKeys)('states independence in en-US (%s)', (key) => {
    const value = (enUSServerMessages as Messages)[key];
    expect(value?.toLowerCase()).toContain('independent');
  });

  it.each(propertyKeys)('states independence in zh-CN (%s)', (key) => {
    const value = (zhCNServerMessages as Messages)[key];
    expect(value).toContain('独立');
  });

  it('en-US telemetry property description cross-references agent.debug', () => {
    expect(enUSServerMessages['server.settings.properties.description.telemetry']).toContain(
      'agent.debug',
    );
  });

  it('zh-CN telemetry property description cross-references agent.debug', () => {
    expect(zhCNServerMessages['server.settings.properties.description.telemetry']).toContain(
      'agent.debug',
    );
  });

  it('en-US agent.debug property description cross-references telemetry.enabled', () => {
    expect(enUSServerMessages['server.settings.properties.description.agentDebugTrace']).toContain(
      'telemetry.enabled',
    );
  });

  it('zh-CN agent.debug property description cross-references telemetry.enabled', () => {
    expect(zhCNServerMessages['server.settings.properties.description.agentDebugTrace']).toContain(
      'telemetry.enabled',
    );
  });

  // Section-level descriptions use cross-reference language (not the word
  // "independent"); pin the cross-reference so a revert to the old standalone
  // wording fails.
  it('telemetry section description cross-references agent.debug in both locales', () => {
    expect(enUSServerMessages['server.settings.sections.telemetry.description']).toContain(
      'agent.debug',
    );
    expect(zhCNServerMessages['server.settings.sections.telemetry.description']).toContain(
      'agent.debug',
    );
  });

  it('agent section description mentions the trace bundle in both locales', () => {
    expect(enUSServerMessages['server.settings.sections.agent.description']).toContain(
      'trace bundle',
    );
    expect(zhCNServerMessages['server.settings.sections.agent.description']).toContain(
      'trace bundle',
    );
  });
});
