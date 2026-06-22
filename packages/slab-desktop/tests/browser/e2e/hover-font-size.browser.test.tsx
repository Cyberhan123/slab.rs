import { page } from 'vitest/browser';
import { Route, Routes } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import type { NodeRendererProps } from 'react-arborist';

import Layout from '@/layouts';
import { WorkspaceTreeRow } from '@/pages/workspace/components/workspace-tree-row';
import type { WorkspaceTreeNode } from '@/pages/workspace/lib/workspace-page-utils';
import { renderDesktopScene } from '../test-utils';

vi.mock('@/pages/plugins/hooks/use-runtime-plugins', () => ({
  useRuntimePlugins: vi.fn<() => unknown>(() => ({
    data: [],
  })),
}));

function RouteMarker() {
  return <div className="p-4">Workspace route</div>;
}

describe('hover font sizing', () => {
  it('keeps shell and workspace row font sizes stable on hover', async () => {
    await renderDesktopScene(
      <Routes>
        <Route element={<Layout />} path="/">
          <Route index element={<RouteMarker />} />
          <Route path="workspace" element={<RouteMarker />} />
        </Route>
      </Routes>,
      { route: '/' },
    );

    await expect.element(page.getByTestId('sidebar-link-workspace')).toBeVisible();
    await expectHoverKeepsFontSize('sidebar-link-workspace');
    await expectHoverKeepsFontSize('header-history-control');

    await renderDesktopScene(
      <WorkspaceTreeRow
        {...workspaceTreeRowProps({
          id: 'src/main.rs',
          name: 'main.rs',
          relativePath: 'src/main.rs',
          kind: 'file',
          hasChildren: false,
        })}
        loadingPaths={new Set()}
        onOpenDirectory={async () => undefined}
        onOpenFile={async () => undefined}
        selectedPath={null}
      />,
    );

    await expect.element(page.getByTestId('workspace-tree-row-src-main-rs')).toBeVisible();
    await expectHoverKeepsFontSize('workspace-tree-row-src-main-rs');
  });
});

async function expectHoverKeepsFontSize(testId: string) {
  const element = document.querySelector<HTMLElement>(`[data-testid="${testId}"]`);
  expect(element).not.toBeNull();
  const before = getComputedStyle(element!).fontSize;

  await page.getByTestId(testId).hover();

  expect(getComputedStyle(element!).fontSize).toBe(before);
}

function workspaceTreeRowProps(
  data: WorkspaceTreeNode,
): NodeRendererProps<WorkspaceTreeNode> {
  return {
    node: {
      data,
      isOpen: false,
      select: vi.fn<() => void>(),
      toggle: vi.fn<() => void>(),
    },
    style: {},
  } as unknown as NodeRendererProps<WorkspaceTreeNode>;
}
