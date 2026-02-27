import api from "@/lib/api";
import useIsTauri from "@/hooks/use-tauri";
import { logTaskId } from "@/lib/api";

const useTranscribe = () => {
    const isTauri = useIsTauri();
    const { isPending, isError, error } = api.useMutation('post', '/v1/audio/transcriptions');

    const handleTranscribe = async (value: File | string): Promise<{ task_id: string }> => {
        let response: { task_id: string };

        if (isTauri) {
            // Tauri mode: use legacy endpoint with file path
            // This is necessary because Tauri can access local filesystem directly
            const legacyResponse = await fetch('/api/v1/audio/transcriptions/legacy', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({ path: value as string }),
            });

            if (!legacyResponse.ok) {
                const errorData = await legacyResponse.json().catch(() => ({ message: 'Unknown error' }));
                throw new Error(errorData.message || 'Transcription request failed');
            }

            response = await legacyResponse.json();
        } else {
            // Web mode: use multipart/form-data upload
            const formData = new FormData();
            formData.append('file', value as File);

            // Use fetch directly for multipart upload
            const uploadResponse = await fetch('/api/v1/audio/transcriptions', {
                method: 'POST',
                body: formData,
            });

            if (!uploadResponse.ok) {
                const errorData = await uploadResponse.json().catch(() => ({ message: 'Unknown error' }));
                throw new Error(errorData.message || 'Transcription request failed');
            }

            response = await uploadResponse.json();
        }

        // Log the task ID for diagnostics
        logTaskId('audio/transcription', response);

        return response;
    }

    return { handleTranscribe, isPending, isError, error };
}

export default useTranscribe;
