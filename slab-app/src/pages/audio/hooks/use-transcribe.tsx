import api from "@/lib/api";
import useIsTauri from "@/hooks/use-tauri";

const useTranscribe = () => {
    const isTauri = useIsTauri();
    const { isPending, isError, error, mutateAsync } = api.useMutation('post', '/v1/audio/transcriptions');

    const handleTranscribe = async (value: File | string): Promise<{ task_id: string }> => {
        if (!isTauri) {
            throw new Error('Web audio upload is not implemented yet. Please use the desktop app.');
        }
        if (typeof value !== 'string' || !value.trim()) {
            throw new Error('Invalid desktop file path.');
        }

        const response = await mutateAsync({
            body: { path: value }
        }) as { task_id: string };

        return response;
    }

    return { handleTranscribe, isPending, isError, error };
}

export default useTranscribe;
