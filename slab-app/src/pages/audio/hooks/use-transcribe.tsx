import api from "@/lib/api";
import useIsTauri from "@/hooks/use-tauri";
const useTranscribe = () => {
    const isTauri = useIsTauri();
    const { isPending, isError, error, mutateAsync } = api.useMutation('post', '/v1/audio/transcriptions');
    const handleTranscribe = async (value: Uint8Array | string): Promise<{ task_id: string }> => {
        return await mutateAsync({
            // web not implemented 
            body: isTauri ? { path: value as string } : { path: "" }
        }) as { task_id: string };
    }
    return { handleTranscribe, isPending, isError, error };
}
export default useTranscribe;