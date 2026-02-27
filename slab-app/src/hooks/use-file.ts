import { open } from '@tauri-apps/plugin-dialog';
import useIsTauri from './use-tauri';

import { ChangeEvent } from 'react';

export type SelectedFile = {
    name?: string;
    file: string | Uint8Array
};
export default function useFile() {
    const isTauri = useIsTauri();
    const handleFile = async (e: ChangeEvent<HTMLInputElement>): Promise<SelectedFile | null> => {
        if (isTauri) {
            const selected = await open({
                multiple: false,
                filters: [{ name: 'Audio', extensions: ['mp3', 'wav', 'flac', 'm4a'] }]
            });

            if (selected && typeof selected === 'string') {
                //TODO: set name
                return { file: selected, name: selected.split('/').pop() };
            }
            return null;
        }
        if (!isTauri) {
            return new Promise(async (resolve) => {
                if (e.target.files && e.target.files[0]) {
                    const file = e.target.files?.[0];
                    if (file) {
                        const buffer = await file.arrayBuffer();
                        resolve({ file: new Uint8Array(buffer), name: file.name });
                    } else {
                        resolve(null);
                    }
                }
            });
            return null;
        }
        return null;
    };

    return { handleFile };
}