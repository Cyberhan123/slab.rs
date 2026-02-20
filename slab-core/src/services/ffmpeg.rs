use anyhow::Result;
use bytemuck::cast_slice;
use ffmpeg_sidecar::{
    command::FfmpegCommand, event::FfmpegEvent, named_pipes::NamedPipe, pipe_name,
};
use std::io::Read;
use tokio::task;
use tracing::info;

const AUDIO_PIPE_NAME: &str = pipe_name!("ffmpeg_audio");

pub async fn read_audio_data(input: String) -> Result<Vec<f32>> {

    task::spawn_blocking(move || -> Result<Vec<f32>> {
        let mut pipe = NamedPipe::new(AUDIO_PIPE_NAME)?;
        info!("[audio] pipe created");

        let input_path = input.clone();
        let ffmpeg_handle = std::thread::spawn(move || -> Result<()> {
            let mut command = FfmpegCommand::new();
            command
                .hide_banner()
                .overwrite()
                .hwaccel("auto")
                .input(&input)
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
                .for_each(|event| {
                    if let FfmpegEvent::Log(level, msg) = event {
                        info!("[FFmpeg {:?}] {}", level, msg);
                    }
                });
            Ok(())
        });


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

        // 等待 FFmpeg 完成
        ffmpeg_handle
            .join()
            .map_err(|_| anyhow::anyhow!("ffmpeg thread panicked while processing: {}", input_path))??;
        let samples: Vec<f32> = cast_slice::<u8, f32>(&buffer).to_vec();
        Ok(samples)
    })
    .await?
}
