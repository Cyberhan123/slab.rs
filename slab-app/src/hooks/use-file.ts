import { open } from '@tauri-apps/plugin-dialog';
import useIsTauri from './use-tauri';

import { ChangeEvent } from 'react';

export type SelectedFile = {
    name?: string;
    file: File | string; // File object for web, string path for Tauri
};

export default function useFile() {
    const isTauri = useIsTauri();

    const handleFile = async (e: ChangeEvent<HTMLInputElement>): Promise<SelectedFile | null> => {
        if (!isTauri) {
            // Web mode: use File object from input
            return new Promise(async (resolve) => {
                if (e.target.files && e.target.files[0]) {
                    const file = e.target.files?.[0];
                    if (file) {
                        resolve({ file, name: file.name });
                    } else {
                        resolve(null);
                    }
                } else {
                    resolve(null);
                }
            });
        }

        if (isTauri) {
            // Tauri mode: open file dialog
            const selected = await open({
                multiple: false,
                filters: [
                    { name: 'Audio', extensions: ['mp3', 'wav', 'flac', 'm4a', 'ogg'] },
                    { name: 'Video', extensions: ['mp4', 'mkv', 'webm'] }
                ]
            });

            if (selected && typeof selected === 'string') {
                return { file: selected, name: selected.split('/').pop() };
            }
            return null;
        }

        return null;
    };

    return { handleFile };
}
