use tokenizers::Tokenizer;

use super::error::CandleLlmError;

pub(crate) struct TokenOutputStream {
    tokenizer: Tokenizer,
    tokens: Vec<u32>,
    prev_index: usize,
    current_index: usize,
}

impl TokenOutputStream {
    pub(crate) fn new(tokenizer: Tokenizer) -> Self {
        Self { tokenizer, tokens: Vec::new(), prev_index: 0, current_index: 0 }
    }

    pub(crate) fn next_token(&mut self, token: u32) -> Result<Option<String>, CandleLlmError> {
        let prev_text = if self.tokens.is_empty() {
            String::new()
        } else {
            self.decode(&self.tokens[self.prev_index..self.current_index])?
        };
        self.tokens.push(token);
        let text = self.decode(&self.tokens[self.prev_index..])?;
        if text.len() > prev_text.len() && text.chars().last().is_some_and(char::is_alphanumeric) {
            let (_, next) = text.split_at(prev_text.len());
            self.prev_index = self.current_index;
            self.current_index = self.tokens.len();
            Ok(Some(next.to_owned()))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn decode_rest(&self) -> Result<Option<String>, CandleLlmError> {
        let prev_text = if self.tokens.is_empty() {
            String::new()
        } else {
            self.decode(&self.tokens[self.prev_index..self.current_index])?
        };
        let text = self.decode(&self.tokens[self.prev_index..])?;
        if text.len() > prev_text.len() {
            let (_, rest) = text.split_at(prev_text.len());
            Ok(Some(rest.to_owned()))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn token_id(&self, token: &str) -> Option<u32> {
        self.tokenizer.get_vocab(true).get(token).copied()
    }

    fn decode(&self, tokens: &[u32]) -> Result<String, CandleLlmError> {
        self.tokenizer.decode(tokens, true).map_err(|error| CandleLlmError::Inference {
            message: format!("token decode: {error}"),
        })
    }
}
