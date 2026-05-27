use super::*;

#[test]
fn audio_speech_post_sse_events_deserialize() {
    let speech_delta: SpeechAudioDeltaEvent = assert_json_deserializes(AUDIO_SPEECH_DELTA_EVENT);
    let speech_done: SpeechAudioDoneEvent = assert_json_deserializes(AUDIO_SPEECH_DONE_EVENT);

    assert_eq!(speech_delta.audio, "c3RyaW5n");
    assert_eq!(speech_done.usage.total_tokens, 3);
}

#[test]
fn audio_transcriptions_post_response_deserializes() {
    let _: CreateTranscription200Response = assert_json_deserializes(AUDIO_TRANSCRIPTION_RESPONSE);
}

#[test]
fn audio_transcriptions_post_diarized_response_deserializes() {
    let diarized: CreateTranscriptionResponseDiarizedJson =
        assert_json_deserializes(AUDIO_TRANSCRIPTION_DIARIZED_RESPONSE);

    assert_eq!(diarized.task, CreateTranscriptionResponseDiarizedJsonTask::Transcribe);
    assert_eq!(diarized.segments.len(), 1);
}

#[test]
fn audio_transcriptions_post_sse_events_deserialize() {
    let transcript_delta: TranscriptTextDeltaEvent =
        assert_json_deserializes(AUDIO_TRANSCRIPT_TEXT_DELTA_EVENT);
    let transcript_done: TranscriptTextDoneEvent =
        assert_json_deserializes(AUDIO_TRANSCRIPT_TEXT_DONE_EVENT);
    let transcript_segment: TranscriptTextSegmentEvent =
        assert_json_deserializes(AUDIO_TRANSCRIPT_TEXT_SEGMENT_EVENT);

    assert_eq!(transcript_delta.delta, "hel");
    assert_eq!(transcript_done.text, "hello world");
    assert_eq!(transcript_segment.speaker, "A");
}

#[test]
fn audio_translations_post_response_deserializes() {
    let translation: CreateTranslation200Response =
        assert_json_deserializes(AUDIO_TRANSLATION_RESPONSE);

    assert!(matches!(translation, CreateTranslation200Response::CreateTranslationResponseJson(_)));
}

#[test]
fn audio_voice_consents_get_collection_response_deserializes() {
    let voice_list: VoiceConsentListResource =
        assert_json_deserializes(VOICE_CONSENT_LIST_RESPONSE);

    assert_eq!(voice_list.data.len(), 1);
}

#[test]
fn audio_voice_consents_item_get_post_delete_payloads_deserialize() {
    let voice_get: VoiceConsentResource = assert_json_deserializes(VOICE_CONSENT_RESPONSE);
    let voice_update: VoiceConsentResource = assert_json_deserializes(VOICE_CONSENT_RESPONSE);
    let voice_delete: VoiceConsentDeletedResource =
        assert_json_deserializes(VOICE_CONSENT_DELETE_RESPONSE);
    let update_request: UpdateVoiceConsentRequest =
        assert_json_deserializes(VOICE_CONSENT_UPDATE_REQUEST);

    assert_eq!(voice_get.id, "cons_1234");
    assert_eq!(voice_update.id, "cons_1234");
    assert!(voice_delete.deleted);
    assert_eq!(update_request.name, "Updated consent name");
}
