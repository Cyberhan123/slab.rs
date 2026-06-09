use std::path::{Path, PathBuf};

use candle_core::{DType, Device, IndexOp, Tensor};
use candle_transformers::generation::LogitsProcessor;
use tokenizers::Tokenizer;

use super::config::{
    CandleLlmLoadConfig, SamplingConfig, TextGenerationRequest, TextGenerationResponse,
    TextGenerationStreamChunk, TextGenerationUsage,
};
use super::error::CandleLlmError;
use super::model::LoadedLlmModel;
use super::prompt::{apply_prompt_format, eos_candidates};
use super::token_stream::TokenOutputStream;
use crate::device::resolve_device;
use crate::runtime::CandleRuntimeEngine;

pub struct CandleLlmEngine {
    model: Option<LoadedLlmModel>,
    tokenizer: Option<Tokenizer>,
    config: Option<CandleLlmLoadConfig>,
    device: Option<Device>,
}

impl CandleLlmEngine {
    pub fn new() -> Self {
        Self { model: None, tokenizer: None, config: None, device: None }
    }

    fn resolve_tokenizer_path(
        model_path: &Path,
        tokenizer_path: Option<PathBuf>,
    ) -> Result<PathBuf, CandleLlmError> {
        if let Some(path) = tokenizer_path {
            return Ok(path);
        }
        let dir = if model_path.is_dir() {
            model_path
        } else {
            model_path.parent().unwrap_or_else(|| Path::new("."))
        };
        let candidate = dir.join("tokenizer.json");
        if candidate.exists() {
            Ok(candidate)
        } else {
            Err(CandleLlmError::InvalidAssetLayout {
                path: dir.display().to_string(),
                message: "missing tokenizer.json".to_owned(),
            })
        }
    }

    fn eos_tokens(stream: &TokenOutputStream, config: &CandleLlmLoadConfig) -> Vec<u32> {
        eos_candidates(config.model_kind)
            .iter()
            .filter_map(|token| stream.token_id(token))
            .collect()
    }

    fn validate_request(request: &TextGenerationRequest) -> Result<(), CandleLlmError> {
        if request.prompt.trim().is_empty() {
            return Err(CandleLlmError::EmptyPrompt);
        }
        if request.max_tokens == 0 {
            return Err(CandleLlmError::InvalidMaxTokens);
        }
        request.sampling.validate()
    }

    pub fn infer_stream<F>(
        &mut self,
        request: TextGenerationRequest,
        mut on_chunk: F,
    ) -> Result<TextGenerationResponse, CandleLlmError>
    where
        F: FnMut(TextGenerationStreamChunk) -> bool,
    {
        let response = self.infer_with_stream(request, |token| {
            on_chunk(TextGenerationStreamChunk::Token(token))
        })?;
        let _ = on_chunk(TextGenerationStreamChunk::Done(response.clone()));
        Ok(response)
    }

    fn infer_with_stream<F>(
        &mut self,
        request: TextGenerationRequest,
        mut on_token: F,
    ) -> Result<TextGenerationResponse, CandleLlmError>
    where
        F: FnMut(String) -> bool,
    {
        Self::validate_request(&request)?;

        let config = self.config.clone().ok_or(CandleLlmError::ModelNotLoaded)?;
        let tokenizer = self.tokenizer.as_ref().ok_or(CandleLlmError::ModelNotLoaded)?;
        let model = self.model.as_mut().ok_or(CandleLlmError::ModelNotLoaded)?;
        let device = self.device.as_ref().ok_or(CandleLlmError::ModelNotLoaded)?;
        model.reset_cache();

        let prompt =
            apply_prompt_format(request.prompt.trim(), config.prompt_format, config.model_kind);
        let prompt_tokens = tokenizer
            .encode(prompt, true)
            .map_err(|error| CandleLlmError::Tokenize { message: error.to_string() })?
            .get_ids()
            .to_vec();
        if prompt_tokens.is_empty() {
            return Err(CandleLlmError::EmptyPrompt);
        }

        let mut all_tokens = prompt_tokens.clone();
        let mut output_stream = TokenOutputStream::new(tokenizer.clone());
        let eos_tokens = Self::eos_tokens(&output_stream, &config);
        let mut logits_processor =
            LogitsProcessor::from_sampling(config.seed, request.sampling.sampling());
        let mut text = String::new();
        let mut finish_reason = "length".to_owned();
        let mut generated = 0usize;

        for index in 0..request.max_tokens {
            let context_size = if index == 0 { all_tokens.len() } else { 1 };
            let start_pos = all_tokens.len().saturating_sub(context_size);
            let input = Tensor::new(&all_tokens[start_pos..], device)
                .and_then(|tensor| tensor.unsqueeze(0))
                .map_err(|error| CandleLlmError::inference(format!("input tensor: {error}")))?;
            let logits = model
                .forward(&input, start_pos)
                .and_then(normalize_logits)
                .map_err(|error| CandleLlmError::inference(format!("forward pass: {error}")))?;
            let logits = apply_repeat_penalty(logits, &all_tokens, &request.sampling)?;
            let next_token = logits_processor
                .sample(&logits)
                .map_err(|error| CandleLlmError::inference(format!("sampling: {error}")))?;

            if !request.ignore_eos && eos_tokens.contains(&next_token) {
                finish_reason = "stop".to_owned();
                break;
            }

            all_tokens.push(next_token);
            generated += 1;
            if let Some(piece) = output_stream.next_token(next_token)? {
                text.push_str(&piece);
                if !on_token(piece) {
                    finish_reason = "stop".to_owned();
                    break;
                }
                if request.stop_sequences.iter().any(|stop| text.ends_with(stop)) {
                    finish_reason = "stop".to_owned();
                    break;
                }
            }
        }

        if let Some(rest) = output_stream.decode_rest()? {
            text.push_str(&rest);
            let _ = on_token(rest);
        }

        Ok(TextGenerationResponse {
            text,
            finish_reason: Some(finish_reason),
            usage: Some(TextGenerationUsage {
                prompt_tokens: prompt_tokens.len() as u32,
                completion_tokens: generated as u32,
                total_tokens: (prompt_tokens.len() + generated) as u32,
            }),
        })
    }
}

impl Default for CandleLlmEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CandleRuntimeEngine for CandleLlmEngine {
    type Error = CandleLlmError;
    type InferenceRequest = TextGenerationRequest;
    type InferenceResponse = TextGenerationResponse;
    type LoadConfig = CandleLlmLoadConfig;

    fn load_model(&mut self, config: Self::LoadConfig) -> Result<(), Self::Error> {
        let tokenizer_path =
            Self::resolve_tokenizer_path(&config.model_path, config.tokenizer_path.clone())?;
        let device = resolve_device(config.device)
            .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))?;
        let model = LoadedLlmModel::load(&config, &device, DType::F32)?;
        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|error| {
            CandleLlmError::LoadTokenizer {
                tokenizer_path: tokenizer_path.display().to_string(),
                message: error.to_string(),
            }
        })?;

        self.model = Some(model);
        self.tokenizer = Some(tokenizer);
        self.config = Some(config);
        self.device = Some(device);
        Ok(())
    }

    fn unload_model(&mut self) {
        self.model = None;
        self.tokenizer = None;
        self.config = None;
        self.device = None;
    }

    fn is_model_loaded(&self) -> bool {
        self.model.is_some()
            && self.tokenizer.is_some()
            && self.config.is_some()
            && self.device.is_some()
    }

    fn infer(
        &mut self,
        request: Self::InferenceRequest,
    ) -> Result<Self::InferenceResponse, Self::Error> {
        self.infer_with_stream(request, |_| true)
    }
}

fn normalize_logits(logits: Tensor) -> candle_core::Result<Tensor> {
    match logits.dims() {
        [_vocab] => Ok(logits),
        [1, _vocab] => logits.squeeze(0),
        [batch, _vocab] => {
            Err(candle_core::Error::Msg(format!("expected batch size 1 for logits, got {batch}")))
        }
        [1, seq_len, _vocab] => logits.i((0, seq_len - 1)),
        [batch, _seq_len, _vocab] => {
            Err(candle_core::Error::Msg(format!("expected batch size 1 for logits, got {batch}")))
        }
        dims => Err(candle_core::Error::Msg(format!("unsupported logits shape {dims:?}"))),
    }
}

fn apply_repeat_penalty(
    logits: Tensor,
    all_tokens: &[u32],
    sampling: &SamplingConfig,
) -> Result<Tensor, CandleLlmError> {
    if (sampling.repeat_penalty - 1.0).abs() < f32::EPSILON {
        return Ok(logits);
    }
    let start_at = all_tokens.len().saturating_sub(sampling.repeat_last_n);
    candle_transformers::utils::apply_repeat_penalty(
        &logits,
        sampling.repeat_penalty,
        &all_tokens[start_at..],
    )
    .map_err(|error| CandleLlmError::inference(format!("repeat penalty: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_engine_is_unloaded() {
        assert!(!CandleLlmEngine::new().is_model_loaded());
    }

    #[test]
    fn explicit_tokenizer_path_is_used_without_filesystem_check() {
        let tokenizer = PathBuf::from("tokenizer.json");
        let resolved = CandleLlmEngine::resolve_tokenizer_path(
            Path::new("missing.gguf"),
            Some(tokenizer.clone()),
        )
        .expect("explicit tokenizer path should resolve");
        assert_eq!(resolved, tokenizer);
    }
}
