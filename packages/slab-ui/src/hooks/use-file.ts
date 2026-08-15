import { useCallback, type ChangeEvent } from 'react';

import { useSlab } from '../provider/slab-provider';
import useIsTauri from './use-tauri';

export type SelectedFile = {
    name?: string;
    file: File | string; // File object for web, string path for Tauri
};

const MEDIA_FILTERS = [
    { name: 'Audio', extensions: ['mp3', 'wav', 'flac', 'm4a', 'ogg'] },
    { name: 'Video', extensions: ['mp4', 'mkv', 'webm'] },
];

export default function useFile() {
    const isTauri = useIsTauri();
    const { ports } = useSlab();

    const handleFile = useCallback(
        async (e?: ChangeEvent<HTMLInputElement>): Promise<SelectedFile | null> => {
            if (isTauri) {
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
        [isTauri, ports.fileDialog],
    );

    return { handleFile };
}
