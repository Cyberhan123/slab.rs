import api from "@/lib/api";
import useIsTauri from "@/hooks/use-tauri";

export type TranscribeVadSettings = {
    enabled: true;
    model_path: string;
    threshold?: number;
    min_speech_duration_ms?: number;
    min_silence_duration_ms?: number;
    max_speech_duration_s?: number;
    speech_pad_ms?: number;
    samples_overlap?: number;
};

export type TranscribeOptions = {
    vad?: TranscribeVadSettings;
    decode?: {
        offset_ms?: number;
        duration_ms?: number;
        no_context?: boolean;
        no_timestamps?: boolean;
        token_timestamps?: boolean;
        split_on_word?: boolean;
        suppress_nst?: boolean;
        word_thold?: number;
        max_len?: number;
        max_tokens?: number;
        temperature?: number;
        temperature_inc?: number;
        entropy_thold?: number;
        logprob_thold?: number;
        no_speech_thold?: number;
        tdrz_enable?: boolean;
    };
};

const useTranscribe = () => {
    const isTauri = useIsTauri();
    const { isPending, isError, error, mutateAsync } = api.useMutation('post', '/v1/audio/transcriptions');

    const handleTranscribe = async (
        value: File | string,
        options?: TranscribeOptions
    ): Promise<{ operation_id: string }> => {
        if (!isTauri) {
            throw new Error('Web audio upload is not implemented yet. Please use the desktop app.');
        }
        if (typeof value !== 'string' || !value.trim()) {
            throw new Error('Invalid desktop file path.');
        }

        const body: {
            path: string;
            vad?: TranscribeVadSettings;
            decode?: TranscribeOptions["decode"];
        } = { path: value };

        if (options?.vad) {
            body.vad = options.vad;
        }
        if (options?.decode) {
            body.decode = options.decode;
        }

        const response = await mutateAsync({
            body
        }) as { operation_id: string };

        return response;
    }

    return { handleTranscribe, isPending, isError, error };
}

export default useTranscribe;
