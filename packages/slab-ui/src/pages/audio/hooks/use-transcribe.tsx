import api from "@slab/api";
import { useTranslation } from "@slab/i18n";
import { useSlab } from "@slab/ui/provider/slab-provider";

import { buildTranscriptionBody } from "../lib/transcribe-body";

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
    model_id?: string;
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
    const isTauri = useSlab().ports.platformInfo.desktop;
    const { isPending, isError, error, mutateAsync } = api.useMutation('post', '/v1/audio/transcriptions', {
        meta: {
            skipGlobalErrorToast: true,
        },
    });

    const handleTranscribe = async (
        value: File | string,
        options?: TranscribeOptions
    ): Promise<{ operation_id: string }> => {
        const body = buildTranscriptionBody(value, options, isTauri, t);
        const response = await mutateAsync({
            body,
        });

        return response;
    }

    return { handleTranscribe, isPending, isError, error };
}

export default useTranscribe;
