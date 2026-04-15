use crate::whisper_grammar::{WhisperGrammarElement, WhisperGrammarElementType};
use crate::whisper_vad::WhisperVadParams;
use crate::{Whisper, WhisperError};
use serde::{Deserialize, Serialize};
use slab_whisper_sys::whisper_token;
use std::ffi::{CStr, CString, c_char, c_float, c_int};
use std::path::PathBuf;
use std::ptr;

/// The sampling strategy to use to pick tokens from a list of likely possibilities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SamplingStrategy {
    /// Greedy sampling.
    Greedy {
        /// Defaults to 5 in `whisper.cpp`.
        best_of: c_int,
    },
    /// Beam search.
    BeamSearch {
        /// Defaults to 5 in `whisper.cpp`.
        beam_size: c_int,
        /// Defaults to -1.0 in `whisper.cpp`.
        patience: c_float,
    },
}

impl Default for SamplingStrategy {
    fn default() -> Self {
        Self::Greedy { best_of: 5 }
    }
}

impl SamplingStrategy {
    fn to_native_strategy(&self) -> slab_whisper_sys::whisper_sampling_strategy {
        match self {
            SamplingStrategy::Greedy { .. } => {
                slab_whisper_sys::whisper_sampling_strategy_WHISPER_SAMPLING_GREEDY
            }
            SamplingStrategy::BeamSearch { .. } => {
                slab_whisper_sys::whisper_sampling_strategy_WHISPER_SAMPLING_BEAM_SEARCH
            }
        }
    }

    fn apply_to_native(&self, fp: &mut slab_whisper_sys::whisper_full_params) {
        fp.strategy = self.to_native_strategy();

        match self {
            SamplingStrategy::Greedy { best_of } => {
                fp.greedy.best_of = (*best_of).max(1);
            }
            SamplingStrategy::BeamSearch { beam_size, patience } => {
                fp.beam_search.beam_size = (*beam_size).max(1);
                fp.beam_search.patience = *patience;
            }
        }
    }

    fn from_native(fp: &slab_whisper_sys::whisper_full_params) -> Self {
        match fp.strategy {
            slab_whisper_sys::whisper_sampling_strategy_WHISPER_SAMPLING_GREEDY => {
                Self::Greedy { best_of: fp.greedy.best_of }
            }
            slab_whisper_sys::whisper_sampling_strategy_WHISPER_SAMPLING_BEAM_SEARCH => {
                Self::BeamSearch {
                    beam_size: fp.beam_search.beam_size,
                    patience: fp.beam_search.patience,
                }
            }
            _ => Self::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SegmentCallbackData {
    pub segment: i32,
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub text: String,
}

/// Stable Rust-native full inference parameters shared across the runtime chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FullParams {
    #[serde(default)]
    pub strategy: SamplingStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_threads: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_max_text_ctx: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_ms: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translate: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_context: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_timestamps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_segment: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_special: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_progress: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_realtime: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_timestamps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_timestamps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thold_pt: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thold_ptsum: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_len: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_on_word: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_ctx: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tdrz_enable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carry_initial_prompt: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompt_tokens: Vec<whisper_token>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_language: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_blank: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_nst: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_initial_ts: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_penalty: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_inc: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entropy_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprob_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_speech_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar: Option<Vec<Vec<WhisperGrammarElement>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i_start_rule: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar_penalty: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vad: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vad_model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vad_params: Option<WhisperVadParams>,
}

impl FullParams {
    pub fn new(strategy: SamplingStrategy) -> Self {
        Self { strategy, ..Self::default() }
    }

    pub fn try_enable_vad(&mut self, vad: bool) -> Result<(), WhisperError> {
        if vad && self.vad_model_path.is_none() {
            return Err(WhisperError::VadModelPathNotSet);
        }

        self.vad = Some(vad);
        Ok(())
    }

    pub(crate) fn from_native(fp: slab_whisper_sys::whisper_full_params) -> Self {
        Self {
            strategy: SamplingStrategy::from_native(&fp),
            n_threads: Some(fp.n_threads),
            n_max_text_ctx: Some(fp.n_max_text_ctx),
            offset_ms: Some(fp.offset_ms),
            duration_ms: Some(fp.duration_ms),
            translate: Some(fp.translate),
            no_context: Some(fp.no_context),
            no_timestamps: Some(fp.no_timestamps),
            single_segment: Some(fp.single_segment),
            print_special: Some(fp.print_special),
            print_progress: Some(fp.print_progress),
            print_realtime: Some(fp.print_realtime),
            print_timestamps: Some(fp.print_timestamps),
            token_timestamps: Some(fp.token_timestamps),
            thold_pt: Some(fp.thold_pt),
            thold_ptsum: Some(fp.thold_ptsum),
            max_len: Some(fp.max_len),
            split_on_word: Some(fp.split_on_word),
            max_tokens: Some(fp.max_tokens),
            debug_mode: Some(fp.debug_mode),
            audio_ctx: Some(fp.audio_ctx),
            tdrz_enable: Some(fp.tdrz_enable),
            suppress_regex: copy_c_string(fp.suppress_regex),
            initial_prompt: copy_c_string(fp.initial_prompt),
            carry_initial_prompt: Some(fp.carry_initial_prompt),
            prompt_tokens: copy_prompt_tokens(fp.prompt_tokens, fp.prompt_n_tokens),
            language: copy_c_string(fp.language),
            detect_language: Some(fp.detect_language),
            suppress_blank: Some(fp.suppress_blank),
            suppress_nst: Some(fp.suppress_nst),
            temperature: Some(fp.temperature),
            max_initial_ts: Some(fp.max_initial_ts),
            length_penalty: Some(fp.length_penalty),
            temperature_inc: Some(fp.temperature_inc),
            entropy_thold: Some(fp.entropy_thold),
            logprob_thold: Some(fp.logprob_thold),
            no_speech_thold: Some(fp.no_speech_thold),
            grammar: copy_grammar_rules(fp.grammar_rules, fp.n_grammar_rules),
            i_start_rule: Some(fp.i_start_rule),
            grammar_penalty: Some(fp.grammar_penalty),
            vad: Some(fp.vad),
            vad_model_path: copy_c_string(fp.vad_model_path).map(PathBuf::from),
            vad_params: Some(WhisperVadParams::from_native(fp.vad_params)),
        }
    }
}

pub(crate) struct InnerFullParams {
    pub(crate) fp: slab_whisper_sys::whisper_full_params,
    suppress_regex: Option<CString>,
    initial_prompt: Option<CString>,
    language: Option<CString>,
    vad_model_path: Option<CString>,
    prompt_tokens: Vec<whisper_token>,
    grammar_rules: Vec<Vec<slab_whisper_sys::whisper_grammar_element>>,
    grammar_rule_ptrs: Vec<*const slab_whisper_sys::whisper_grammar_element>,
}

impl Clone for InnerFullParams {
    fn clone(&self) -> Self {
        let mut cloned = Self {
            fp: self.fp,
            suppress_regex: self.suppress_regex.clone(),
            initial_prompt: self.initial_prompt.clone(),
            language: self.language.clone(),
            vad_model_path: self.vad_model_path.clone(),
            prompt_tokens: self.prompt_tokens.clone(),
            grammar_rules: self.grammar_rules.clone(),
            grammar_rule_ptrs: Vec::new(),
        };
        cloned.sync_backing();
        cloned
    }
}

impl std::fmt::Debug for InnerFullParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerFullParams").finish_non_exhaustive()
    }
}

impl Whisper {
    pub fn new_full_params(&self, sampling_strategy: SamplingStrategy) -> FullParams {
        let fp =
            unsafe { self.lib.whisper_full_default_params(sampling_strategy.to_native_strategy()) };
        FullParams::from_native(fp)
    }
}

impl InnerFullParams {
    pub(crate) fn from_canonical(
        lib: &slab_whisper_sys::WhisperLib,
        value: &FullParams,
    ) -> Result<Self, WhisperError> {
        let mut inner = Self {
            fp: unsafe { lib.whisper_full_default_params(value.strategy.to_native_strategy()) },
            suppress_regex: None,
            initial_prompt: None,
            language: None,
            vad_model_path: None,
            prompt_tokens: value.prompt_tokens.clone(),
            grammar_rules: Vec::new(),
            grammar_rule_ptrs: Vec::new(),
        };

        value.strategy.apply_to_native(&mut inner.fp);

        if let Some(n_threads) = value.n_threads {
            inner.fp.n_threads = n_threads;
        }
        if let Some(n_max_text_ctx) = value.n_max_text_ctx {
            inner.fp.n_max_text_ctx = n_max_text_ctx;
        }
        if let Some(offset_ms) = value.offset_ms {
            inner.fp.offset_ms = offset_ms;
        }
        if let Some(duration_ms) = value.duration_ms {
            inner.fp.duration_ms = duration_ms;
        }
        if let Some(translate) = value.translate {
            inner.fp.translate = translate;
        }
        if let Some(no_context) = value.no_context {
            inner.fp.no_context = no_context;
        }
        if let Some(no_timestamps) = value.no_timestamps {
            inner.fp.no_timestamps = no_timestamps;
        }
        if let Some(single_segment) = value.single_segment {
            inner.fp.single_segment = single_segment;
        }
        if let Some(print_special) = value.print_special {
            inner.fp.print_special = print_special;
        }
        if let Some(print_progress) = value.print_progress {
            inner.fp.print_progress = print_progress;
        }
        if let Some(print_realtime) = value.print_realtime {
            inner.fp.print_realtime = print_realtime;
        }
        if let Some(print_timestamps) = value.print_timestamps {
            inner.fp.print_timestamps = print_timestamps;
        }
        if let Some(token_timestamps) = value.token_timestamps {
            inner.fp.token_timestamps = token_timestamps;
        }
        if let Some(thold_pt) = value.thold_pt {
            inner.fp.thold_pt = thold_pt;
        }
        if let Some(thold_ptsum) = value.thold_ptsum {
            inner.fp.thold_ptsum = thold_ptsum;
        }
        if let Some(max_len) = value.max_len {
            inner.fp.max_len = max_len;
        }
        if let Some(split_on_word) = value.split_on_word {
            inner.fp.split_on_word = split_on_word;
        }
        if let Some(max_tokens) = value.max_tokens {
            inner.fp.max_tokens = max_tokens;
        }
        if let Some(debug_mode) = value.debug_mode {
            inner.fp.debug_mode = debug_mode;
        }
        if let Some(audio_ctx) = value.audio_ctx {
            inner.fp.audio_ctx = audio_ctx;
        }
        if let Some(tdrz_enable) = value.tdrz_enable {
            inner.fp.tdrz_enable = tdrz_enable;
        }
        if let Some(carry_initial_prompt) = value.carry_initial_prompt {
            inner.fp.carry_initial_prompt = carry_initial_prompt;
        }
        if let Some(detect_language) = value.detect_language {
            inner.fp.detect_language = detect_language;
        }
        if let Some(suppress_blank) = value.suppress_blank {
            inner.fp.suppress_blank = suppress_blank;
        }
        if let Some(suppress_nst) = value.suppress_nst {
            inner.fp.suppress_nst = suppress_nst;
        }
        if let Some(temperature) = value.temperature {
            inner.fp.temperature = temperature;
        }
        if let Some(max_initial_ts) = value.max_initial_ts {
            inner.fp.max_initial_ts = max_initial_ts;
        }
        if let Some(length_penalty) = value.length_penalty {
            inner.fp.length_penalty = length_penalty;
        }
        if let Some(temperature_inc) = value.temperature_inc {
            inner.fp.temperature_inc = temperature_inc;
        }
        if let Some(entropy_thold) = value.entropy_thold {
            inner.fp.entropy_thold = entropy_thold;
        }
        if let Some(logprob_thold) = value.logprob_thold {
            inner.fp.logprob_thold = logprob_thold;
        }
        if let Some(no_speech_thold) = value.no_speech_thold {
            inner.fp.no_speech_thold = no_speech_thold;
        }
        if let Some(grammar_penalty) = value.grammar_penalty {
            inner.fp.grammar_penalty = grammar_penalty;
        }
        if let Some(vad) = value.vad {
            inner.fp.vad = vad;
        }

        if let Some(suppress_regex) = value.suppress_regex.as_deref() {
            inner.suppress_regex = Some(CString::new(suppress_regex)?);
        }
        if let Some(initial_prompt) = value.initial_prompt.as_deref() {
            inner.initial_prompt = Some(CString::new(initial_prompt)?);
        }
        if let Some(language) = value.language.as_deref() {
            inner.language = Some(CString::new(language)?);
        }
        if let Some(vad_model_path) = value.vad_model_path.as_ref() {
            inner.vad_model_path = Some(CString::new(vad_model_path.to_string_lossy().as_ref())?);
        }
        if let Some(vad_params) = value.vad_params.as_ref() {
            let mut native = unsafe { lib.whisper_vad_default_params() };
            vad_params.apply_to(&mut native);
            inner.fp.vad_params = native;
        }
        if let Some(grammar) = value.grammar.as_ref() {
            inner.grammar_rules = grammar
                .iter()
                .map(|rule| rule.iter().copied().map(WhisperGrammarElement::to_c_type).collect())
                .collect();
        }
        if let Some(i_start_rule) = value.i_start_rule {
            inner.fp.i_start_rule = i_start_rule;
        }

        inner.sync_backing();

        if inner.fp.vad && inner.vad_model_path.is_none() {
            return Err(WhisperError::VadModelPathNotSet);
        }

        Ok(inner)
    }

    fn sync_backing(&mut self) {
        self.fp.suppress_regex =
            self.suppress_regex.as_ref().map_or(ptr::null(), |value| value.as_ptr());
        self.fp.initial_prompt =
            self.initial_prompt.as_ref().map_or(ptr::null(), |value| value.as_ptr());
        self.fp.language = self.language.as_ref().map_or(ptr::null(), |value| value.as_ptr());
        self.fp.vad_model_path =
            self.vad_model_path.as_ref().map_or(ptr::null(), |value| value.as_ptr());

        self.fp.prompt_tokens =
            if self.prompt_tokens.is_empty() { ptr::null() } else { self.prompt_tokens.as_ptr() };
        self.fp.prompt_n_tokens = self.prompt_tokens.len().min(i32::MAX as usize) as c_int;

        self.grammar_rule_ptrs.clear();
        self.grammar_rule_ptrs.extend(self.grammar_rules.iter().map(|rule| rule.as_ptr()));
        self.fp.grammar_rules = if self.grammar_rule_ptrs.is_empty() {
            ptr::null_mut()
        } else {
            self.grammar_rule_ptrs.as_mut_ptr()
        };
        self.fp.n_grammar_rules = self.grammar_rule_ptrs.len();
    }
}

fn copy_c_string(ptr: *const c_char) -> Option<String> {
    (!ptr.is_null()).then(|| unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned())
}

fn copy_prompt_tokens(ptr: *const whisper_token, count: c_int) -> Vec<whisper_token> {
    if ptr.is_null() || count <= 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(ptr, count as usize) }.to_vec()
    }
}

fn copy_grammar_rules(
    ptr: *mut *const slab_whisper_sys::whisper_grammar_element,
    count: usize,
) -> Option<Vec<Vec<WhisperGrammarElement>>> {
    if ptr.is_null() || count == 0 {
        return None;
    }

    let rules = unsafe { std::slice::from_raw_parts(ptr, count) }
        .iter()
        .copied()
        .filter(|rule_ptr| !rule_ptr.is_null())
        .map(|rule_ptr| {
            let mut items = Vec::new();
            let mut offset = 0usize;

            loop {
                let element = unsafe { *rule_ptr.add(offset) };
                items.push(WhisperGrammarElement {
                    element_type: WhisperGrammarElementType::from(element.type_),
                    value: element.value,
                });
                offset += 1;

                if element.type_ == slab_whisper_sys::whisper_gretype_WHISPER_GRETYPE_END {
                    break;
                }
            }

            items
        })
        .collect::<Vec<_>>();

    Some(rules)
}

// concurrent usage is prevented by &mut self on methods that modify the struct
unsafe impl Send for InnerFullParams {}
unsafe impl Sync for InnerFullParams {}
