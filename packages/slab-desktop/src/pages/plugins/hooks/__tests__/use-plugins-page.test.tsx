import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

const { parsePluginPackManifestMock } = vi.hoisted(() => ({
  parsePluginPackManifestMock: vi.fn<(file: File) => Promise<null>>(),
}));

vi.mock("@slab/i18n", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock("@/hooks/use-global-header-meta", () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderSearch: vi.fn<() => void>(),
}));

vi.mock("@slab/api", () => ({
  default: {
    useMutation: () => ({
      mutateAsync: vi.fn<() => Promise<void>>(),
    }),
    useQuery: () => ({
      data: [],
      error: null,
      isFetching: false,
      isLoading: false,
      refetch: vi.fn<() => Promise<void>>(),
    }),
  },
  getLocalizedErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : String(error),
  postFormData: vi.fn<(path: string, file: File) => Promise<unknown>>(),
}));

vi.mock("../../lib/plugin-manifest-preview", () => ({
  parsePluginPackManifest: parsePluginPackManifestMock,
}));

vi.mock("../../lib/plugin-runtime-client", () => ({
  connectPluginEvents: vi.fn<() => () => void>(() => () => {}),
}));

import { usePluginsPage } from "../use-plugins-page";

function wrapper({ children }: { children: ReactNode }) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
    },
  });
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}

describe("usePluginsPage", () => {
  it("treats a null import preview as a reviewable parse failure", async () => {
    parsePluginPackManifestMock.mockResolvedValueOnce(null);
    const { result } = renderHook(() => usePluginsPage(), { wrapper });

    await act(async () => {
      result.current.handleImportFileChange(new File(["not a zip"], "broken.plugin.slab"));
    });

    await waitFor(() => expect(result.current.importPreviewFailed).toBe(true));
    expect(result.current.importPreview).toBeNull();
    expect(result.current.canImport).toBe(false);

    act(() => {
      result.current.setHasReviewedPermissions(true);
    });

    expect(result.current.canImport).toBe(true);
  });
});
