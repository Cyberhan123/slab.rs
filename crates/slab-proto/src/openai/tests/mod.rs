use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::openai::audio::{
    SpeechAudioDeltaEvent, SpeechAudioDoneEvent, UpdateVoiceConsentRequest,
    VoiceConsentDeletedResource, VoiceConsentListResource, VoiceConsentResource,
};
use crate::openai::{
    ChatCompletionDeleted, CompactResource, CreateChatCompletionRequest,
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse, CreateCompletionRequest,
    CreateCompletionRequestModel, CreateCompletionResponse, CreateEmbeddingRequest,
    CreateEmbeddingResponse, CreateTranscription200Response,
    CreateTranscriptionResponseDiarizedJson, CreateTranscriptionResponseDiarizedJsonTask,
    CreateTranslation200Response, CreateVideoEditJsonBody, CreateVideoExtendJsonBody,
    CreateVideoJsonBody, CreateVideoRemixBody, DeletedSkillResource, DeletedSkillVersionResource,
    ImagesResponse, ListModelsResponse, Model, Response, ResponseCompletedEvent,
    ResponseCreatedEvent, ResponseErrorEvent, ResponseFailedEvent,
    ResponseFunctionCallArgumentsDeltaEvent, ResponseFunctionCallArgumentsDoneEvent,
    ResponseInProgressEvent, ResponseIncompleteEvent, ResponseItemList, ResponseQueuedEvent,
    ResponseStatus, ResponseTextDeltaEvent, ResponseTextDoneEvent, SkillListResource,
    SkillResource, SkillVersionListResource, SkillVersionResource, TranscriptTextDeltaEvent,
    TranscriptTextDoneEvent, TranscriptTextSegmentEvent, UpdateChatCompletionRequest,
    VideoCharacterResource, VideoContentVariant, VideoListResource, VideoResource,
};

const MODELS_LIST_RESPONSE: &str = r#"{"object":"list","data":[{"id":"string","created":0,"object":"model","owned_by":"string"}]}"#;
const MODEL_RESPONSE: &str = r#"{"id":"string","created":0,"object":"model","owned_by":"string"}"#;
const AUDIO_TRANSCRIPTION_RESPONSE: &str = r#"{"text":"string"}"#;
const AUDIO_TRANSCRIPTION_DIARIZED_RESPONSE: &str = r#"{"task":"transcribe","duration":12.34,"text":"hello world","segments":[{"type":"transcript.text.segment","id":"seg_1","start":0.0,"end":1.2,"text":"hello","speaker":"A"}],"usage":{"type":"tokens","input_tokens":12,"output_tokens":4,"total_tokens":16}}"#;
const AUDIO_TRANSLATION_RESPONSE: &str = r#"{"text":"string"}"#;
const VOICE_CONSENT_LIST_RESPONSE: &str = r#"{"object":"list","data":[{"object":"audio.voice_consent","id":"cons_1234","name":"string","language":"string","created_at":0}],"first_id":"string","last_id":"string","has_more":true}"#;
const VOICE_CONSENT_RESPONSE: &str = r#"{"object":"audio.voice_consent","id":"cons_1234","name":"string","language":"string","created_at":0}"#;
const VOICE_CONSENT_DELETE_RESPONSE: &str =
    r#"{"id":"cons_1234","object":"audio.voice_consent","deleted":true}"#;
const VOICE_CONSENT_UPDATE_REQUEST: &str = r#"{"name":"Updated consent name"}"#;
const CHAT_COMPLETION_RESPONSE: &str = r#"{"id":"string","choices":[{"finish_reason":"stop","index":0,"message":{"content":"string","refusal":"string","tool_calls":[{"id":"string","type":"function","function":{"name":"string","arguments":"string"}}],"annotations":[{"type":"url_citation","url_citation":{"end_index":0,"start_index":0,"url":"http://example.com","title":"string"}}],"role":"assistant","function_call":{"arguments":"string","name":"string"},"audio":{"id":"string","expires_at":0,"data":"string","transcript":"string"}},"logprobs":{"content":[{"token":"string","logprob":0,"bytes":[0],"top_logprobs":[{"token":"string","logprob":0,"bytes":[0]}]}],"refusal":[{"token":"string","logprob":0,"bytes":[0],"top_logprobs":[{"token":"string","logprob":0,"bytes":[0]}]}]}}],"created":0,"model":"string","service_tier":"auto","system_fingerprint":"string","object":"chat.completion","usage":{"completion_tokens":0,"prompt_tokens":0,"total_tokens":0,"completion_tokens_details":{"accepted_prediction_tokens":0,"audio_tokens":0,"reasoning_tokens":0,"rejected_prediction_tokens":0},"prompt_tokens_details":{"audio_tokens":0,"cached_tokens":0}}}"#;
const CHAT_COMPLETION_DELETE_RESPONSE: &str =
    r#"{"object":"chat.completion.deleted","id":"string","deleted":true}"#;
const CHAT_COMPLETION_UPDATE_REQUEST: &str = r#"{"metadata":{"foo":"string"}}"#;
const CHAT_COMPLETION_STREAM_CHUNK: &str = r#"{"id":"chatcmpl_123","choices":[{"delta":{"role":"assistant","content":"Hel"},"finish_reason":"stop","index":0}],"created":0,"model":"gpt-4.1","object":"chat.completion.chunk"}"#;
const CHAT_COMPLETION_RESPONSE_CONTENT_FILTER: &str = r#"{"id":"chatcmpl_filter","choices":[{"finish_reason":"content_filter","index":0,"message":{"content":"","refusal":"safety blocked","tool_calls":[],"annotations":[],"role":"assistant","function_call":{"arguments":"{}","name":"noop"},"audio":{"id":"aud_1","expires_at":0,"data":"","transcript":""}},"logprobs":null}],"created":0,"model":"gpt-4.1","service_tier":null,"system_fingerprint":"fp_test","object":"chat.completion","usage":{"completion_tokens":0,"prompt_tokens":0,"total_tokens":0,"completion_tokens_details":{"accepted_prediction_tokens":0,"audio_tokens":0,"reasoning_tokens":0,"rejected_prediction_tokens":0},"prompt_tokens_details":{"audio_tokens":0,"cached_tokens":0}}}"#;
const CHAT_COMPLETION_REQUEST_OPTIONALS: &str = r#"{"messages":[{"role":"user","content":"hello"}],"model":"gpt-4.1","metadata":null,"service_tier":null,"modalities":null,"stream_options":null,"temperature":0.2,"logprobs":true,"stream":true}"#;
const COMPLETIONS_REQUEST: &str =
    r#"{"model":"gpt-3.5-turbo-instruct","prompt":"Write a haiku about tests."}"#;
const COMPLETIONS_REQUEST_CUSTOM_MODEL: &str = r#"{"model":"my-custom-model","prompt":"test"}"#;
const COMPLETIONS_RESPONSE: &str = r#"{"id":"cmpl_123","object":"text_completion","created":0,"model":"gpt-3.5-turbo-instruct","choices":[{"text":"Tests keep systems safe.","index":0,"finish_reason":"stop","logprobs":null}],"usage":{"prompt_tokens":4,"completion_tokens":5,"total_tokens":9}}"#;
const COMPLETIONS_RESPONSE_CONTENT_FILTER: &str = r#"{"id":"cmpl_124","object":"text_completion","created":0,"model":"gpt-3.5-turbo-instruct","choices":[{"text":"","index":0,"finish_reason":"content_filter","logprobs":null}],"usage":{"prompt_tokens":2,"completion_tokens":0,"total_tokens":2}}"#;
const EMBEDDINGS_RESPONSE: &str = r#"{"data":[{"index":0,"embedding":[-3.4028236692093846E+38],"object":"embedding"}],"model":"string","object":"list","usage":{"prompt_tokens":0,"total_tokens":0}}"#;
const EMBEDDINGS_REQUEST: &str = r#"{"input":"hello","model":"text-embedding-3-small"}"#;
const IMAGES_RESPONSE: &str = r#"{"created":0,"data":[{"b64_json":"string","url":"http://example.com","revised_prompt":"string"}],"background":"transparent","output_format":"png","size":"1024x1024","quality":"low","usage":{"input_tokens":0,"total_tokens":0,"output_tokens":0,"output_tokens_details":{"image_tokens":0,"text_tokens":0},"input_tokens_details":{"text_tokens":0,"image_tokens":0}}}"#;
const VIDEO_RESOURCE_RESPONSE: &str = r#"{"id":"video_123","object":"video","model":{},"status":"completed","progress":100,"created_at":0,"completed_at":1,"expires_at":2,"prompt":"A forest in spring","size":"720x1280","seconds":"4","remixed_from_video_id":null,"error":null}"#;
const VIDEO_LIST_RESPONSE: &str = r#"{"object":"list","data":[{"id":"video_123","object":"video","model":{},"status":"completed","progress":100,"created_at":0,"completed_at":1,"expires_at":2,"prompt":"A forest in spring","size":"720x1280","seconds":"4","remixed_from_video_id":null,"error":null}],"first_id":"video_123","last_id":"video_123","has_more":false}"#;
const VIDEO_CREATE_REQUEST: &str = r#"{"prompt":"Create a cinematic drone shot."}"#;
const VIDEO_EDIT_REQUEST: &str =
    r#"{"video":{"id":"video_123"},"prompt":"Make the scene at sunset."}"#;
const VIDEO_EXTEND_REQUEST: &str =
    r#"{"video":{"id":"video_123"},"prompt":"Continue the shot to the right.","seconds":"8"}"#;
const VIDEO_REMIX_REQUEST: &str = r#"{"prompt":"Remix with watercolor style."}"#;
const VIDEO_CHARACTER_RESPONSE: &str = r#"{"id":"char_001","name":"Ava","created_at":0}"#;
const VIDEO_CONTENT_VARIANT: &str = r#""thumbnail""#;
const RESPONSE_RESOURCE: &str = r#"{"id":"resp_67ccd3a9da748190baa7f1570fe91ac604becb25c45c1d41","object":"response","created_at":1741476777,"status":"completed","completed_at":1741476778,"error":null,"incomplete_details":null,"instructions":null,"max_output_tokens":null,"model":"gpt-4o-2024-08-06","output":[{"type":"message","id":"msg_67ccd3acc8d48190a77525dc6de64b4104becb25c45c1d41","status":"completed","role":"assistant","content":[{"type":"output_text","text":"The image depicts a scenic landscape with a wooden boardwalk or pathway leading through lush, green grass under a blue sky with some clouds. The setting suggests a peaceful natural area, possibly a park or nature reserve. There are trees and shrubs in the background.","annotations":[]}]}],"parallel_tool_calls":true,"previous_response_id":null,"reasoning":{"effort":null,"summary":null},"store":true,"temperature":1,"text":{"format":{"type":"text"}},"tool_choice":"auto","tools":[],"top_p":1,"truncation":"disabled","usage":{"input_tokens":328,"input_tokens_details":{"cached_tokens":0},"output_tokens":52,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":380},"user":null,"metadata":{}}"#;
const RESPONSE_INPUT_ITEMS: &str = r#"{"object":"list","data":[{"type":"message","role":"user","status":"in_progress","content":[{"type":"input_text","text":"string"}],"id":"string"}],"has_more":true,"first_id":"string","last_id":"string"}"#;
const RESPONSE_DELETE_BODY: &str = "";
const RESPONSE_INPUT_TOKENS: &str = r#"{"object":"response.input_tokens","input_tokens":123}"#;
const RESPONSE_COMPACT: &str = r#"{"id":"resp_001","object":"response.compaction","output":[{"type":"message","role":"user","content":[{"type":"input_text","text":"Summarize our launch checklist from last week."}]},{"type":"message","role":"user","content":[{"type":"input_text","text":"You are performing a CONTEXT CHECKPOINT COMPACTION..."}]},{"type":"compaction","id":"cmp_001","encrypted_content":"encrypted-summary"}],"created_at":1731459200,"usage":{"input_tokens":42897,"output_tokens":12000,"total_tokens":54912}}"#;
const SKILL_LIST_RESPONSE: &str = r#"{"object":"list","data":[{"id":"string","object":"skill","name":"string","description":"string","created_at":0,"default_version":"string","latest_version":"string"}],"first_id":"string","last_id":"string","has_more":true}"#;
const SKILL_RESPONSE: &str = r#"{"id":"string","object":"skill","name":"string","description":"string","created_at":0,"default_version":"string","latest_version":"string"}"#;
const SKILL_DELETE_RESPONSE: &str = r#"{"object":"skill.deleted","deleted":true,"id":"string"}"#;
const SKILL_VERSION_LIST_RESPONSE: &str = r#"{"object":"list","data":[{"object":"skill.version","id":"string","skill_id":"string","version":"string","created_at":0,"name":"string","description":"string"}],"first_id":"string","last_id":"string","has_more":true}"#;
const SKILL_VERSION_RESPONSE: &str = r#"{"object":"skill.version","id":"string","skill_id":"string","version":"string","created_at":0,"name":"string","description":"string"}"#;
const SKILL_VERSION_DELETE_RESPONSE: &str =
    r#"{"object":"skill.version.deleted","deleted":true,"id":"string","version":"string"}"#;
const SKILL_CONTENT_RAW: &[u8] = b"string";
const SKILL_VERSION_CONTENT_RAW: &[u8] = b"string";
const AUDIO_SPEECH_DELTA_EVENT: &str = r#"{"type":"speech.audio.delta","audio":"c3RyaW5n"}"#;
const AUDIO_SPEECH_DONE_EVENT: &str =
    r#"{"type":"speech.audio.done","usage":{"input_tokens":1,"output_tokens":2,"total_tokens":3}}"#;
const AUDIO_TRANSCRIPT_TEXT_DELTA_EVENT: &str =
    r#"{"type":"transcript.text.delta","delta":"hel","segment_id":"seg_1"}"#;
const AUDIO_TRANSCRIPT_TEXT_DONE_EVENT: &str = r#"{"type":"transcript.text.done","text":"hello world","usage":{"type":"tokens","input_tokens":12,"output_tokens":4,"total_tokens":16}}"#;
const AUDIO_TRANSCRIPT_TEXT_SEGMENT_EVENT: &str = r#"{"type":"transcript.text.segment","id":"seg_1","start":0.0,"end":1.2,"text":"hello","speaker":"A"}"#;
const RESPONSE_QUEUED_EVENT: &str = r#"{"type":"response.queued","response":{"id":"resp_67ccd3a9da748190baa7f1570fe91ac604becb25c45c1d41","object":"response","created_at":1741476777,"status":"completed","completed_at":1741476778,"error":null,"incomplete_details":null,"instructions":null,"max_output_tokens":null,"model":"gpt-4o-2024-08-06","output":[{"type":"message","id":"msg_67ccd3acc8d48190a77525dc6de64b4104becb25c45c1d41","status":"completed","role":"assistant","content":[{"type":"output_text","text":"The image depicts a scenic landscape with a wooden boardwalk or pathway leading through lush, green grass under a blue sky with some clouds. The setting suggests a peaceful natural area, possibly a park or nature reserve. There are trees and shrubs in the background.","annotations":[]}]}],"parallel_tool_calls":true,"previous_response_id":null,"reasoning":{"effort":null,"summary":null},"store":true,"temperature":1,"text":{"format":{"type":"text"}},"tool_choice":"auto","tools":[],"top_p":1,"truncation":"disabled","usage":{"input_tokens":328,"input_tokens_details":{"cached_tokens":0},"output_tokens":52,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":380},"user":null,"metadata":{}},"sequence_number":3}"#;
const RESPONSE_IN_PROGRESS_EVENT: &str = r#"{"type":"response.in_progress","response":{"id":"resp_67ccd3a9da748190baa7f1570fe91ac604becb25c45c1d41","object":"response","created_at":1741476777,"status":"completed","completed_at":1741476778,"error":null,"incomplete_details":null,"instructions":null,"max_output_tokens":null,"model":"gpt-4o-2024-08-06","output":[{"type":"message","id":"msg_67ccd3acc8d48190a77525dc6de64b4104becb25c45c1d41","status":"completed","role":"assistant","content":[{"type":"output_text","text":"The image depicts a scenic landscape with a wooden boardwalk or pathway leading through lush, green grass under a blue sky with some clouds. The setting suggests a peaceful natural area, possibly a park or nature reserve. There are trees and shrubs in the background.","annotations":[]}]}],"parallel_tool_calls":true,"previous_response_id":null,"reasoning":{"effort":null,"summary":null},"store":true,"temperature":1,"text":{"format":{"type":"text"}},"tool_choice":"auto","tools":[],"top_p":1,"truncation":"disabled","usage":{"input_tokens":328,"input_tokens_details":{"cached_tokens":0},"output_tokens":52,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":380},"user":null,"metadata":{}},"sequence_number":4}"#;
const RESPONSE_INCOMPLETE_EVENT: &str = r#"{"type":"response.incomplete","response":{"id":"resp_67ccd3a9da748190baa7f1570fe91ac604becb25c45c1d41","object":"response","created_at":1741476777,"status":"completed","completed_at":1741476778,"error":null,"incomplete_details":null,"instructions":null,"max_output_tokens":null,"model":"gpt-4o-2024-08-06","output":[{"type":"message","id":"msg_67ccd3acc8d48190a77525dc6de64b4104becb25c45c1d41","status":"completed","role":"assistant","content":[{"type":"output_text","text":"The image depicts a scenic landscape with a wooden boardwalk or pathway leading through lush, green grass under a blue sky with some clouds. The setting suggests a peaceful natural area, possibly a park or nature reserve. There are trees and shrubs in the background.","annotations":[]}]}],"parallel_tool_calls":true,"previous_response_id":null,"reasoning":{"effort":null,"summary":null},"store":true,"temperature":1,"text":{"format":{"type":"text"}},"tool_choice":"auto","tools":[],"top_p":1,"truncation":"disabled","usage":{"input_tokens":328,"input_tokens_details":{"cached_tokens":0},"output_tokens":52,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":380},"user":null,"metadata":{}},"sequence_number":5}"#;
const RESPONSE_FAILED_EVENT: &str = r#"{"type":"response.failed","sequence_number":6,"response":{"id":"resp_67ccd3a9da748190baa7f1570fe91ac604becb25c45c1d41","object":"response","created_at":1741476777,"status":"completed","completed_at":1741476778,"error":null,"incomplete_details":null,"instructions":null,"max_output_tokens":null,"model":"gpt-4o-2024-08-06","output":[{"type":"message","id":"msg_67ccd3acc8d48190a77525dc6de64b4104becb25c45c1d41","status":"completed","role":"assistant","content":[{"type":"output_text","text":"The image depicts a scenic landscape with a wooden boardwalk or pathway leading through lush, green grass under a blue sky with some clouds. The setting suggests a peaceful natural area, possibly a park or nature reserve. There are trees and shrubs in the background.","annotations":[]}]}],"parallel_tool_calls":true,"previous_response_id":null,"reasoning":{"effort":null,"summary":null},"store":true,"temperature":1,"text":{"format":{"type":"text"}},"tool_choice":"auto","tools":[],"top_p":1,"truncation":"disabled","usage":{"input_tokens":328,"input_tokens_details":{"cached_tokens":0},"output_tokens":52,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":380},"user":null,"metadata":{}}}"#;
const RESPONSE_ERROR_EVENT: &str =
    r#"{"type":"error","code":null,"message":"stream aborted","param":null,"sequence_number":7}"#;
const RESPONSE_OUTPUT_TEXT_DELTA_EVENT: &str = r#"{"type":"response.output_text.delta","item_id":"msg_1","output_index":0,"content_index":0,"delta":"hel","sequence_number":8,"logprobs":[]}"#;
const RESPONSE_OUTPUT_TEXT_DONE_EVENT: &str = r#"{"type":"response.output_text.done","item_id":"msg_1","output_index":0,"content_index":0,"text":"hello","sequence_number":9,"logprobs":[]}"#;
const RESPONSE_FUNCTION_CALL_ARGS_DELTA_EVENT: &str = r#"{"type":"response.function_call_arguments.delta","item_id":"fc_1","output_index":0,"sequence_number":10,"delta":"{\"city\":\"Sh\"}"}"#;
const RESPONSE_FUNCTION_CALL_ARGS_DONE_EVENT: &str = r#"{"type":"response.function_call_arguments.done","item_id":"fc_1","name":"get_weather","output_index":0,"sequence_number":11,"arguments":"{\"city\":\"Shanghai\"}"}"#;

#[derive(Debug, Deserialize)]
struct InputTokensResponse {
    object: String,
    input_tokens: i32,
}

fn assert_json_deserializes<T>(fixture: &str) -> T
where
    T: DeserializeOwned,
{
    serde_json::from_str(fixture)
        .unwrap_or_else(|error| panic!("failed to deserialize fixture: {error}\n{fixture}"))
}

mod audio;
mod chat;
mod completions;
mod embeddings;
mod exports;
mod images;
mod models_api;
mod responses_api;
mod skills;
mod videos;
