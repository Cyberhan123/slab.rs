import { create } from 'zustand';

import type { DownloadTrackingState } from '../lib/model-download';

type HubModelDownloadState = {
  downloadTracking: Record<string, DownloadTrackingState>;
  setDownloadTracking: (modelId: string, next: DownloadTrackingState | null) => void;
};

export const useHubModelDownloadStore = create<HubModelDownloadState>((set) => ({
  downloadTracking: {},
  setDownloadTracking: (modelId, next) => {
    const trimmedModelId = modelId.trim();

    if (!trimmedModelId) {
      return;
    }

    set((state) => {
      if (next) {
        return {
          downloadTracking: {
            ...state.downloadTracking,
            [trimmedModelId]: next,
          },
        };
      }

      if (!(trimmedModelId in state.downloadTracking)) {
        return state;
      }

      const { [trimmedModelId]: _removed, ...rest } = state.downloadTracking;
      return { downloadTracking: rest };
    });
  },
}));
