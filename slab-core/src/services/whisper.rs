use crate::services::ffmpeg;
use anyhow::Result;
use slab_whisper::{
    SamplingStrategy, Whisper, WhisperContext, WhisperContextParameters,
};
use std::sync::{Arc, Mutex, OnceLock};
use subparse::{ SubtitleEntry , timetypes::{TimeSpan, TimePoint}};

static INSTANCE: OnceLock<WhisperService> = OnceLock::new();
pub struct WhisperService {
    instance: Arc<Whisper>,
    ctx: Arc<Mutex<Option<WhisperContext>>>,
}

impl WhisperService {
    pub fn init(path: String) -> &'static Self {
        INSTANCE.get_or_init(|| {
            let whisper = Whisper::new(path).expect("load lib failed");
            Self {
                instance: Arc::new(whisper),
                ctx: Arc::new(Mutex::new(None)),
            }
        })
    }

    pub fn get_instance(&self) -> Arc<Whisper> {
        Arc::clone(&self.instance)
    }

    pub fn new_context(&self, path_to_model: String, params: WhisperContextParameters) -> Result<()> {
        let mut ctx_lock = self.ctx.lock().unwrap();
        let old_ctx = ctx_lock.take();

        drop(old_ctx);

        let ctx = self
            .instance
            .new_context_with_params(&path_to_model, params)?;
        *ctx_lock = Some(ctx);

        Ok(())
    }

    pub async fn inference(&self, path: String) -> Result<Vec<SubtitleEntry>> {
        let ctx_lock = self.ctx.lock().unwrap();

        if ctx_lock.is_none() {
            return Err(anyhow::anyhow!("context not initialized"));
        }

        let ctx = ctx_lock.as_ref().ok_or_else(|| anyhow::anyhow!("context not initialized"))?;

        let params = self.instance.new_full_params(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });

        let audio_data = ffmpeg::read_audio_data(path).await?;
        let mut state = ctx.create_state()?;
        state
            .full(params, &audio_data[..])
            .map_err(|e| anyhow::anyhow!("failed to run model: {:?}", e))?;

        let srt_entries: Vec<SubtitleEntry> = state
            .as_iter()
            .map(|segment| {
                SubtitleEntry {
                    timespan: TimeSpan::new(
                        // 从厘秒转换为毫秒
                        TimePoint::from_msecs(segment.start_timestamp()*10),
                        TimePoint::from_msecs(segment.end_timestamp()*10),
                    ),
                    line: Some(segment.to_string().trim().to_string()),
                }
            })
            .collect();
        Ok(srt_entries)
    }


}


#[cfg(test)]
mod test {
    // use super::*;

    use slab_whisper::Whisper;

    #[test]
    fn test_whisper() {
        Whisper::new("D:\\Code\\Rust\\slab.rs\\slab-whisper\\models\\ggml-small.bin").expect("load lib failed");
    }
}
