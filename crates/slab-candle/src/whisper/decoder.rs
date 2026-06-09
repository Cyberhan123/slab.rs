use candle_core::{D, IndexOp, Tensor};
use candle_nn::ops::{log_softmax, softmax};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::whisper;
use tokenizers::Tokenizer;

use super::config::{TranscriptionRequest, TranscriptionSegment, WhisperTask};
use super::error::CandleWhisperError;
use super::model::WhisperModel;

const LANGUAGE_CODES: [&str; 99] = [
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it",
    "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur",
    "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn",
    "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si",
    "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo",
    "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
    "ha", "ba", "jw", "su",
];

#[derive(Debug, Clone)]
struct DecodingResult {
    tokens: Vec<u32>,
    text: String,
    avg_logprob: f64,
    no_speech_prob: f64,
    compression_ratio: f64,
}

pub(crate) struct WhisperDecoder {
    tokenizer: Tokenizer,
    sot_token: u32,
    transcribe_token: u32,
    translate_token: u32,
    eot_token: u32,
    no_speech_token: u32,
    no_timestamps_token: u32,
    suppress_tokens: Vec<u32>,
}

impl WhisperDecoder {
    pub(crate) fn new(
        tokenizer: Tokenizer,
        model: &WhisperModel,
        timestamps: bool,
    ) -> Result<Self, CandleWhisperError> {
        let no_timestamps_token = token_id(&tokenizer, whisper::NO_TIMESTAMPS_TOKEN)?;
        let mut suppress_tokens = model.config().suppress_tokens.clone();
        if timestamps {
            suppress_tokens.push(no_timestamps_token);
        }
        let no_speech_token = whisper::NO_SPEECH_TOKENS
            .iter()
            .find_map(|token| token_id(&tokenizer, token).ok())
            .ok_or_else(|| CandleWhisperError::Inference {
                message: "unable to find a no-speech token".to_owned(),
            })?;
        Ok(Self {
            sot_token: token_id(&tokenizer, whisper::SOT_TOKEN)?,
            transcribe_token: token_id(&tokenizer, whisper::TRANSCRIBE_TOKEN)?,
            translate_token: token_id(&tokenizer, whisper::TRANSLATE_TOKEN)?,
            eot_token: token_id(&tokenizer, whisper::EOT_TOKEN)?,
            no_speech_token,
            no_timestamps_token,
            suppress_tokens,
            tokenizer,
        })
    }

    pub(crate) fn decode(
        &self,
        model: &mut WhisperModel,
        mel: &Tensor,
        request: &TranscriptionRequest,
    ) -> Result<(String, Option<String>, Vec<TranscriptionSegment>), CandleWhisperError> {
        let (_, _, content_frames) = mel
            .dims3()
            .map_err(|error| CandleWhisperError::inference(format!("mel dims: {error}")))?;
        let mut seek = 0usize;
        let mut language_token = None;
        let mut detected_language = None;
        let mut segments = Vec::new();

        while seek < content_frames {
            let segment_size = usize::min(content_frames - seek, whisper::N_FRAMES);
            let mel_segment = mel.narrow(2, seek, segment_size).map_err(|error| {
                CandleWhisperError::inference(format!("mel segment slice: {error}"))
            })?;
            let audio_features = model.encoder_forward(&mel_segment).map_err(|error| {
                CandleWhisperError::inference(format!("encode audio segment: {error}"))
            })?;
            if language_token.is_none() {
                language_token =
                    self.resolve_language_token(model, &mel_segment, &audio_features, request)?;
                detected_language =
                    language_token.and_then(|token| language_from_token(&self.tokenizer, token));
            }

            let decoded = self.decode_with_fallback(
                model,
                &mel_segment,
                &audio_features,
                language_token,
                request,
            )?;
            seek += segment_size;
            if decoded.no_speech_prob > no_speech_threshold(request)
                && decoded.avg_logprob < logprob_threshold(request)
            {
                continue;
            }

            let start_ms = ((seek - segment_size) as f64 * whisper::HOP_LENGTH as f64 * 1000.0
                / whisper::SAMPLE_RATE as f64)
                .round() as u32;
            let duration_ms = (segment_size as f64 * whisper::HOP_LENGTH as f64 * 1000.0
                / whisper::SAMPLE_RATE as f64)
                .round() as u32;
            segments.extend(self.build_segments(start_ms, duration_ms, &decoded, request)?);
        }

        let text = segments.iter().map(|segment| segment.text.as_str()).collect::<String>();
        Ok((text, detected_language, segments))
    }

    fn decode_with_fallback(
        &self,
        model: &mut WhisperModel,
        mel: &Tensor,
        audio_features: &Tensor,
        language_token: Option<u32>,
        request: &TranscriptionRequest,
    ) -> Result<DecodingResult, CandleWhisperError> {
        let temperatures = fallback_temperatures(request);
        let last_index = temperatures.len().saturating_sub(1);
        let mut last_error = None;
        for (index, temperature) in temperatures.into_iter().enumerate() {
            match self.decode_segment(
                model,
                mel,
                audio_features,
                language_token,
                request,
                temperature,
            ) {
                Ok(decoded) => {
                    let needs_fallback = decoded.compression_ratio
                        > compression_ratio_threshold(request)
                        || decoded.avg_logprob < logprob_threshold(request);
                    if index == last_index
                        || !needs_fallback
                        || decoded.no_speech_prob > no_speech_threshold(request)
                    {
                        return Ok(decoded);
                    }
                }
                Err(error) => {
                    if index == last_index {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| CandleWhisperError::Inference {
            message: "temperature fallback produced no decoding result".to_owned(),
        }))
    }

    fn decode_segment(
        &self,
        model: &mut WhisperModel,
        mel: &Tensor,
        audio_features: &Tensor,
        language_token: Option<u32>,
        request: &TranscriptionRequest,
        temperature: f64,
    ) -> Result<DecodingResult, CandleWhisperError> {
        let mut tokens = self.initial_tokens(language_token, request)?;
        let sample_begin = tokens.len();
        let max_tokens = request.max_tokens.unwrap_or(model.config().max_target_positions / 2);
        let sampling =
            if temperature < 1e-7 { Sampling::ArgMax } else { Sampling::All { temperature } };
        let mut logits_processor =
            LogitsProcessor::from_sampling(request.seed.unwrap_or(0), sampling);
        let mut sum_logprob = 0f64;
        let mut no_speech_prob = f64::NAN;

        for index in 0..max_tokens {
            let token_ids = tokens.iter().map(|token| *token as i64).collect::<Vec<_>>();
            let input = Tensor::new(token_ids.as_slice(), mel.device())
                .and_then(|tensor| tensor.unsqueeze(0))
                .map_err(|error| CandleWhisperError::inference(format!("token tensor: {error}")))?;
            let decoder_output = model
                .decoder_forward(&input, audio_features, index == 0)
                .map_err(|error| CandleWhisperError::inference(format!("decode token: {error}")))?;
            if index == 0 {
                let logits = model
                    .final_linear(&decoder_output.i((..1, ..1, ..)).map_err(|error| {
                        CandleWhisperError::inference(format!("no-speech logits slice: {error}"))
                    })?)
                    .and_then(|tensor| tensor.i((0, 0)))
                    .map_err(|error| {
                        CandleWhisperError::inference(format!("no-speech logits: {error}"))
                    })?;
                no_speech_prob = softmax(&logits, D::Minus1)
                    .and_then(|tensor| tensor.i(self.no_speech_token as usize))
                    .and_then(|tensor| tensor.to_scalar::<f32>())
                    .map_err(|error| {
                        CandleWhisperError::inference(format!("no-speech probability: {error}"))
                    })? as f64;
            }
            let logits =
                model.last_token_logits(&decoder_output, tokens.len()).map_err(|error| {
                    CandleWhisperError::inference(format!("project logits: {error}"))
                })?;
            let logits = if request.timestamps {
                self.apply_timestamp_rules(&logits, &tokens, language_token, request)?
            } else {
                logits
            };
            let logits = self.apply_suppress_tokens(logits)?;
            let next_token = logits_processor
                .sample(&logits)
                .map_err(|error| CandleWhisperError::inference(format!("sample token: {error}")))?;
            tokens.push(next_token);
            let prob = softmax(&logits, D::Minus1)
                .and_then(|tensor| tensor.i(next_token as usize))
                .and_then(|tensor| tensor.to_scalar::<f32>())
                .map_err(|error| {
                    CandleWhisperError::inference(format!("sample probability: {error}"))
                })? as f64;
            if next_token == self.eot_token || tokens.len() > model.config().max_target_positions {
                break;
            }
            sum_logprob += prob.max(f64::MIN_POSITIVE).ln();
        }

        let generated = tokens[sample_begin..]
            .iter()
            .copied()
            .filter(|token| *token != self.eot_token)
            .collect::<Vec<_>>();
        let text = self.decode_text(&generated)?;
        let avg_logprob =
            if generated.is_empty() { 0.0 } else { sum_logprob / generated.len() as f64 };
        let compression_ratio = compression_ratio(&text);
        Ok(DecodingResult {
            tokens: generated,
            text,
            avg_logprob,
            no_speech_prob,
            compression_ratio,
        })
    }

    fn initial_tokens(
        &self,
        language_token: Option<u32>,
        request: &TranscriptionRequest,
    ) -> Result<Vec<u32>, CandleWhisperError> {
        let mut tokens = vec![self.sot_token];
        if let Some(language_token) = language_token {
            tokens.push(language_token);
        }
        match request.task {
            WhisperTask::Transcribe => tokens.push(self.transcribe_token),
            WhisperTask::Translate => tokens.push(self.translate_token),
        }
        if !request.timestamps {
            tokens.push(self.no_timestamps_token);
        }
        if let Some(prompt) = request.prompt.as_deref()
            && !prompt.trim().is_empty()
        {
            let prompt_ids = self
                .tokenizer
                .encode(prompt, true)
                .map_err(|error| CandleWhisperError::Inference {
                    message: format!("prompt tokenization failed: {error}"),
                })?
                .get_ids()
                .to_vec();
            tokens.extend(prompt_ids);
        }
        Ok(tokens)
    }

    fn resolve_language_token(
        &self,
        model: &mut WhisperModel,
        mel: &Tensor,
        audio_features: &Tensor,
        request: &TranscriptionRequest,
    ) -> Result<Option<u32>, CandleWhisperError> {
        if let Some(language) = request.language.as_deref()
            && language != "auto"
        {
            return Ok(Some(token_id(&self.tokenizer, &format!("<|{language}|>"))?));
        }
        if request.detect_language || request.language.as_deref() == Some("auto") {
            return self.detect_language(model, mel, audio_features).map(Some);
        }
        Ok(None)
    }

    fn detect_language(
        &self,
        model: &mut WhisperModel,
        mel: &Tensor,
        audio_features: &Tensor,
    ) -> Result<u32, CandleWhisperError> {
        let language_token_ids = LANGUAGE_CODES
            .iter()
            .map(|code| token_id(&self.tokenizer, &format!("<|{code}|>")))
            .collect::<Result<Vec<_>, _>>()?;
        let tokens = Tensor::new(&[self.sot_token as i64], mel.device())
            .and_then(|tensor| tensor.unsqueeze(0))
            .map_err(|error| {
                CandleWhisperError::inference(format!("language token tensor: {error}"))
            })?;
        let decoder_output = model
            .decoder_forward(&tokens, audio_features, true)
            .map_err(|error| CandleWhisperError::inference(format!("language decode: {error}")))?;
        let ids = Tensor::new(language_token_ids.as_slice(), mel.device())
            .map_err(|error| CandleWhisperError::inference(format!("language ids: {error}")))?;
        let logits = model
            .final_linear(&decoder_output.i((.., 0, ..)).map_err(|error| {
                CandleWhisperError::inference(format!("language logits slice: {error}"))
            })?)
            .and_then(|tensor| tensor.squeeze(0))
            .and_then(|tensor| tensor.index_select(&ids, 0))
            .map_err(|error| CandleWhisperError::inference(format!("language logits: {error}")))?;
        let probs = softmax(&logits, D::Minus1)
            .and_then(|tensor| tensor.to_vec1::<f32>())
            .map_err(|error| CandleWhisperError::inference(format!("language softmax: {error}")))?;
        let best = probs
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.total_cmp(right))
            .map(|(index, _)| index)
            .ok_or_else(|| {
                CandleWhisperError::inference("language detection produced no scores")
            })?;
        Ok(language_token_ids[best])
    }

    fn apply_suppress_tokens(&self, logits: Tensor) -> Result<Tensor, CandleWhisperError> {
        let device = logits.device().clone();
        let mut logits = logits.to_vec1::<f32>().map_err(|error| {
            CandleWhisperError::inference(format!("logits to vec for suppression: {error}"))
        })?;
        for token in &self.suppress_tokens {
            if let Some(value) = logits.get_mut(*token as usize) {
                *value = f32::NEG_INFINITY;
            }
        }
        Tensor::new(logits.as_slice(), &device)
            .map_err(|error| CandleWhisperError::inference(format!("suppressed logits: {error}")))
    }

    fn apply_timestamp_rules(
        &self,
        input_logits: &Tensor,
        tokens: &[u32],
        language_token: Option<u32>,
        request: &TranscriptionRequest,
    ) -> Result<Tensor, CandleWhisperError> {
        let device = input_logits.device();
        let timestamp_begin = self.no_timestamps_token + 1;
        let vocab_size = input_logits.dim(0).map_err(|error| {
            CandleWhisperError::inference(format!("timestamp logits dim: {error}"))
        })? as u32;
        let sample_begin = if language_token.is_some() { 3 } else { 2 };
        let sampled_tokens =
            if tokens.len() > sample_begin { &tokens[sample_begin..] } else { &[] };
        let mut logits = input_logits.clone();

        if !sampled_tokens.is_empty() {
            let last_was_timestamp =
                sampled_tokens.last().map(|token| *token >= timestamp_begin).unwrap_or(false);
            let penultimate_was_timestamp = sampled_tokens
                .get(sampled_tokens.len().saturating_sub(2))
                .map(|token| *token >= timestamp_begin)
                .unwrap_or(false);

            if last_was_timestamp {
                let mask = if penultimate_was_timestamp {
                    timestamp_mask(vocab_size, |token| token >= timestamp_begin)
                } else {
                    timestamp_mask(vocab_size, |token| token < self.eot_token)
                };
                logits = logits
                    .broadcast_add(&Tensor::new(mask.as_slice(), device).map_err(|error| {
                        CandleWhisperError::inference(format!("timestamp mask: {error}"))
                    })?)
                    .map_err(|error| {
                        CandleWhisperError::inference(format!("timestamp mask apply: {error}"))
                    })?;
            }

            let timestamp_tokens = sampled_tokens
                .iter()
                .copied()
                .filter(|token| *token >= timestamp_begin)
                .collect::<Vec<_>>();
            if let Some(last_timestamp) = timestamp_tokens.last() {
                let timestamp_last = if last_was_timestamp && !penultimate_was_timestamp {
                    *last_timestamp
                } else {
                    last_timestamp + 1
                };
                let mask = timestamp_mask(vocab_size, |token| {
                    token >= timestamp_begin && token < timestamp_last
                });
                logits = logits
                    .broadcast_add(&Tensor::new(mask.as_slice(), device).map_err(|error| {
                        CandleWhisperError::inference(format!("timestamp order mask: {error}"))
                    })?)
                    .map_err(|error| {
                        CandleWhisperError::inference(format!("timestamp order apply: {error}"))
                    })?;
            }
        }

        if tokens.len() == sample_begin {
            let mask = timestamp_mask(vocab_size, |token| token < timestamp_begin);
            logits = logits
                .broadcast_add(&Tensor::new(mask.as_slice(), device).map_err(|error| {
                    CandleWhisperError::inference(format!("initial timestamp mask: {error}"))
                })?)
                .map_err(|error| {
                    CandleWhisperError::inference(format!("initial timestamp apply: {error}"))
                })?;
            if let Some(max_initial_timestamp_index) = request.max_initial_timestamp_index {
                let last_allowed = timestamp_begin + max_initial_timestamp_index;
                let mask = timestamp_mask(vocab_size, |token| token > last_allowed);
                logits = logits
                    .broadcast_add(&Tensor::new(mask.as_slice(), device).map_err(|error| {
                        CandleWhisperError::inference(format!(
                            "max initial timestamp mask: {error}"
                        ))
                    })?)
                    .map_err(|error| {
                        CandleWhisperError::inference(format!(
                            "max initial timestamp apply: {error}"
                        ))
                    })?;
            }
        }

        let log_probs = log_softmax(&logits, D::Minus1).map_err(|error| {
            CandleWhisperError::inference(format!("timestamp logsoftmax: {error}"))
        })?;
        let timestamp_logprob = log_probs
            .narrow(0, timestamp_begin as usize, vocab_size as usize - timestamp_begin as usize)
            .and_then(|tensor| tensor.log_sum_exp(D::Minus1))
            .and_then(|tensor| tensor.to_scalar::<f32>())
            .map_err(|error| {
                CandleWhisperError::inference(format!("timestamp logprob: {error}"))
            })?;
        let max_text_logprob = log_probs
            .narrow(0, 0, timestamp_begin as usize)
            .and_then(|tensor| tensor.max(D::Minus1))
            .and_then(|tensor| tensor.to_scalar::<f32>())
            .map_err(|error| {
                CandleWhisperError::inference(format!("timestamp text logprob: {error}"))
            })?;
        if timestamp_logprob > max_text_logprob {
            let mask = timestamp_mask(vocab_size, |token| token < timestamp_begin);
            logits = logits
                .broadcast_add(&Tensor::new(mask.as_slice(), device).map_err(|error| {
                    CandleWhisperError::inference(format!("timestamp preference mask: {error}"))
                })?)
                .map_err(|error| {
                    CandleWhisperError::inference(format!("timestamp preference apply: {error}"))
                })?;
        }

        Ok(logits)
    }

    fn decode_text(&self, tokens: &[u32]) -> Result<String, CandleWhisperError> {
        let text_tokens = tokens
            .iter()
            .copied()
            .filter(|token| {
                *token != self.eot_token && !is_timestamp_token(*token, self.no_timestamps_token)
            })
            .collect::<Vec<_>>();
        self.tokenizer.decode(&text_tokens, true).map_err(|error| CandleWhisperError::Inference {
            message: format!("decode text failed: {error}"),
        })
    }

    fn build_segments(
        &self,
        start_ms: u32,
        duration_ms: u32,
        decoded: &DecodingResult,
        request: &TranscriptionRequest,
    ) -> Result<Vec<TranscriptionSegment>, CandleWhisperError> {
        if !request.timestamps {
            return Ok(vec![TranscriptionSegment {
                start_ms,
                end_ms: start_ms + duration_ms,
                text: decoded.text.clone(),
                tokens: decoded.tokens.clone(),
            }]);
        }

        let mut segments = Vec::new();
        let mut tokens_to_decode = Vec::new();
        let mut segment_start_ms = start_ms;
        for token in &decoded.tokens {
            if is_timestamp_token(*token, self.no_timestamps_token) {
                let relative_ms = timestamp_relative_ms(*token, self.no_timestamps_token);
                let segment_end_ms = start_ms + relative_ms;
                if !tokens_to_decode.is_empty() {
                    let text = self.decode_text(&tokens_to_decode)?;
                    segments.push(TranscriptionSegment {
                        start_ms: segment_start_ms,
                        end_ms: segment_end_ms,
                        text,
                        tokens: std::mem::take(&mut tokens_to_decode),
                    });
                }
                segment_start_ms = segment_end_ms;
            } else if *token != self.eot_token {
                tokens_to_decode.push(*token);
            }
        }
        if !tokens_to_decode.is_empty() {
            let text = self.decode_text(&tokens_to_decode)?;
            segments.push(TranscriptionSegment {
                start_ms: segment_start_ms,
                end_ms: start_ms + duration_ms,
                text,
                tokens: tokens_to_decode,
            });
        }
        Ok(segments)
    }
}

pub(crate) fn token_id(tokenizer: &Tokenizer, token: &str) -> Result<u32, CandleWhisperError> {
    tokenizer.token_to_id(token).ok_or_else(|| CandleWhisperError::Inference {
        message: format!("no token id for {token}"),
    })
}

fn fallback_temperatures(request: &TranscriptionRequest) -> Vec<f64> {
    if !request.temperature_fallback.is_empty() {
        return request.temperature_fallback.clone();
    }
    request
        .temperature
        .map_or_else(|| whisper::TEMPERATURES.to_vec(), |temperature| vec![temperature])
}

fn compression_ratio_threshold(request: &TranscriptionRequest) -> f64 {
    request.compression_ratio_threshold.unwrap_or(whisper::COMPRESSION_RATIO_THRESHOLD)
}

fn logprob_threshold(request: &TranscriptionRequest) -> f64 {
    request.logprob_threshold.unwrap_or(whisper::LOGPROB_THRESHOLD)
}

fn no_speech_threshold(request: &TranscriptionRequest) -> f64 {
    request.no_speech_threshold.unwrap_or(whisper::NO_SPEECH_THRESHOLD)
}

fn timestamp_mask(vocab_size: u32, suppress: impl Fn(u32) -> bool) -> Vec<f32> {
    (0..vocab_size).map(|token| if suppress(token) { f32::NEG_INFINITY } else { 0.0 }).collect()
}

fn is_timestamp_token(token: u32, no_timestamps_token: u32) -> bool {
    token > no_timestamps_token
}

fn timestamp_relative_ms(token: u32, no_timestamps_token: u32) -> u32 {
    let timestamp_begin = no_timestamps_token + 1;
    token.saturating_sub(timestamp_begin) * 20
}

fn language_from_token(tokenizer: &Tokenizer, token: u32) -> Option<String> {
    let token = tokenizer.id_to_token(token)?;
    token.strip_prefix("<|").and_then(|value| value.strip_suffix("|>")).map(str::to_owned)
}

fn compression_ratio(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let bytes = text.as_bytes();
    let mut runs = 1usize;
    for window in bytes.windows(2) {
        if window[0] != window[1] {
            runs += 1;
        }
    }
    bytes.len() as f64 / runs as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_tokens_start_after_no_timestamps() {
        assert!(!is_timestamp_token(50363, 50363));
        assert!(is_timestamp_token(50364, 50363));
    }

    #[test]
    fn timestamp_segment_timing_starts_at_zero() {
        assert_eq!(timestamp_relative_ms(50364, 50363), 0);
        assert_eq!(timestamp_relative_ms(50365, 50363), 20);
    }

    #[test]
    fn fallback_uses_default_temperatures() {
        let request = TranscriptionRequest::default();
        assert_eq!(fallback_temperatures(&request), whisper::TEMPERATURES);
    }

    #[test]
    fn explicit_temperature_disables_fallback_ladder() {
        let request =
            TranscriptionRequest { temperature: Some(0.3), ..TranscriptionRequest::default() };
        assert_eq!(fallback_temperatures(&request), vec![0.3]);
    }

    #[test]
    fn compression_ratio_detects_repetition() {
        assert!(compression_ratio("aaaaaa") > compression_ratio("abcdef"));
    }
}
