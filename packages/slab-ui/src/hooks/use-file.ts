import { useCallback, type ChangeEvent } from 'react';

import { useSlab } from '../provider/slab-provider';

export type SelectedFile = {
    name?: string;
    file: File | string; // File object for web, string path for Tauri
};

const MEDIA_FILTERS = [
    { name: 'Audio', extensions: ['mp3', 'wav', 'flac', 'm4a', 'ogg'] },
    { name: 'Video', extensions: ['mp4', 'mkv', 'webm'] },
];

export default function useFile() {
    const { ports } = useSlab();
    const isDesktop = ports.platformInfo.desktop;

    const handleFile = useCallback(
        async (e?: ChangeEvent<HTMLInputElement>): Promise<SelectedFile | null> => {
            if (isDesktop) {
                // Tauri mode: open the native file dialog via the injected port.
                const picked = await ports.fileDialog.pickFile({
                    multiple: false,
                    filters: MEDIA_FILTERS,
                });
                if (picked?.path) {
                    return { file: picked.path, name: picked.name };
                }
                return null;
            }

            // Web mode: use File object from input
            const file = e?.target.files?.[0];
            if (!file) {
                return null;
            }
            return { file, name: file.name };
        },
        [isDesktop, ports.fileDialog],
    );

    return { handleFile };
}
