use crate::engine;
use anyhow;
use anyhow::Result;
use bytemuck::cast_slice;
use ffmpeg_sidecar::{
    command::FfmpegCommand, event::FfmpegEvent, named_pipes::NamedPipe, pipe_name,
};
use std::io::Read;
use std::path::Path;
use std::sync::mpsc;
use thiserror::Error;
use tokio::task;
use tracing::{error, info};

const AUDIO_PIPE_NAME: &str = pipe_name!("ffmpeg_audio");

#[derive(Debug, Error)]
pub enum FFmpegServiceError {
    /// The part file is corrupted
    #[error("Invalid part file - corrupted file")]
    InvalidResume,

    #[error("Thread join error {0}")]
    JoinError(#[from] tokio::task::JoinError),
}

pub struct FfmpegService;

impl FfmpegService {
    pub fn new() -> Self {
        Self {}
    }

    /// Convert video to a format suitable for streaming (e.g., MP4 with H.264 codec).
    pub fn convert_video<P: AsRef<Path>>(
        input_path: P,
        output_path: P,
    ) -> Result<(), engine::EngineError> {
        let input_path = input_path.as_ref().to_path_buf();
        let output_path = output_path.as_ref().to_path_buf();

        let mut command = FfmpegCommand::new();

        command
            .hide_banner()
            .overwrite()
            .args(["-hwaccel", "auto"])
            .input(
                input_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("error audio path"))?,
            )
            .args(["-movflags", "+faststart"])
            .output(
                output_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("error audio path"))?,
            )
            .spawn()?
            .iter()?
            .for_each(|event| match event {
                FfmpegEvent::Log(level, msg) => info!("[FFmpeg {:?}] {}", level, msg),
                FfmpegEvent::Done => info!("FFmpeg finished processing: {}", output_path.display()),
                FfmpegEvent::Error(e) => error!("FFmpeg error: {}", e),
                _ => {}
            });

        Ok(())
    }

    /// Read audio data from a video file and return it as a vector of f32 samples.
    pub async fn read_audio_data<P: AsRef<Path>>(
        input: P,
    ) -> Result<Vec<f32>, engine::EngineError> {
        let mut input_path = input.as_ref().to_path_buf();
        input_path = input_path.components().collect();

        task::spawn_blocking(move || -> Result<Vec<f32>, engine::EngineError> {
            let mut pipe = NamedPipe::new(AUDIO_PIPE_NAME)?;
            info!("[audio] pipe created");
            let (ready_sender, ready_receiver) = mpsc::channel::<()>();

            let ffmpeg_handle = std::thread::spawn(move || -> Result<()> {
                let mut command = FfmpegCommand::new();
                let mut ready_signal_sent = false;
                command
                    .hide_banner()
                    .overwrite()
                    .hwaccel("auto")
                    .input(
                        &input_path
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("error audio path"))?,
                    )
                    .args([
                        "-vn",
                        "-f",
                        "f32le",
                        "-acodec",
                        "pcm_f32le",
                        "-ar",
                        "16000",
                        "-ac",
                        "1",
                    ])
                    .output(AUDIO_PIPE_NAME)
                    .print_command()
                    .spawn()?
                    .iter()?
                    .for_each(|event| match event {
                        FfmpegEvent::Progress(_) if !ready_signal_sent => {
                            let _ = ready_sender.send(());
                            ready_signal_sent = true;
                        }
                        FfmpegEvent::Log(level, msg) => info!("[FFmpeg {:?}] {}", level, msg),
                        FfmpegEvent::Done => {
                            info!("FFmpeg finished processing: {}", &input_path.display())
                        }
                        FfmpegEvent::Error(e) => error!("FFmpeg error: {}", e),
                        _ => {}
                    });

                if !ready_signal_sent {
                    let _ = ready_sender.send(());
                }
                Ok(())
            });

            ready_receiver
                .recv()
                .map_err(|e| anyhow::anyhow!("failed to receive ffmpeg ready signal: {e}"))?;

            let mut buffer = Vec::new();
            let mut chunk = vec![0u8; 8192];

            loop {
                let result = pipe.read(&mut chunk);
                match result {
                    Ok(0) => break, // EOF
                    Ok(bytes_read) => {
                        buffer.extend_from_slice(&chunk[..bytes_read]);
                    }
                    Err(err) => {
                        if err.kind() == std::io::ErrorKind::BrokenPipe {
                            break;
                        } else {
                            return Err(err.into());
                        }
                    }
                }
            }

            // wait for the FFmpeg thread to finish and check for errors
            ffmpeg_handle
                .join()
                .map_err(|_| anyhow::anyhow!("ffmpeg worker thread panicked"))??;

            let samples: Vec<f32> = cast_slice::<u8, f32>(&buffer).to_vec();
            Ok(samples)
        })
        .await
        .map_err(FFmpegServiceError::JoinError)?
    }
}

impl Default for FfmpegService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use tokio;
    use tracing_test::traced_test;
  
    #[tokio::test]
    #[traced_test]
    async fn test_whisper() {
        let mut test_data_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata/samples");
        println!("Current executable path: {:?}", test_data_path);

        let jfk_audio_path = test_data_path.join("jfk.wav");

        let srt_entries = FfmpegService::read_audio_data(jfk_audio_path)
            .await
            .unwrap();

        println!("Read {} audio samples", srt_entries.len());
    }
}
