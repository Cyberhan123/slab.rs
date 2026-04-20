import useIsTauri from "@/hooks/use-tauri";
import api from "@/lib/api";
import type { components } from "@/lib/api/v1.d.ts";
import { useTranslation } from "@slab/i18n";

type AudioTranscriptionRequest = components["schemas"]["AudioTranscriptionRequest"];

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
    language?: string;
    prompt?: string;
    detect_language?: boolean;
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
    const { t } = useTranslation();
    const isTauri = useIsTauri();
    const { isPending, isError, error, mutateAsync } = api.useMutation('post', '/v1/audio/transcriptions');

    const handleTranscribe = async (
        value: File | string,
        options?: TranscribeOptions
    ): Promise<{ operation_id: string }> => {
        if (!isTauri) {
            throw new Error(t('pages.audio.error.webUploadNotImplemented'));
        }
        if (typeof value !== 'string' || !value.trim()) {
            throw new Error(t('pages.audio.error.invalidDesktopFilePath'));
        }

        const body: AudioTranscriptionRequest = { path: value };

        if (typeof options?.language === 'string' && options.language.trim()) {
            body.language = options.language.trim();
        }
        if (typeof options?.prompt === 'string' && options.prompt.trim()) {
            body.prompt = options.prompt.trim();
        }
        if (options?.detect_language) {
            body.detect_language = true;
        }
        if (options?.vad) {
            body.vad = options.vad;
        }
        if (options?.decode) {
            body.decode = options.decode;
        }

        const response = await mutateAsync({
            body,
        });

        return response;
    }

    return { handleTranscribe, isPending, isError, error };
}

export default useTranscribe;
