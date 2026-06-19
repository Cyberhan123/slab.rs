use super::AnyStore;
use crate::domain::models::{
    AudioTranscriptionResultData, ImageGenerationResultData, TaskResult, TaskStatus,
    VideoGenerationResultData, task_progress_from_payload,
};
use crate::infra::db::entities::{
    AudioTranscriptionTaskRecord, AudioTranscriptionTaskViewRecord, ImageGenerationTaskRecord,
    ImageGenerationTaskViewRecord, MediaTaskState, NewAudioTranscriptionTaskRecord,
    NewImageGenerationTaskRecord, NewVideoGenerationTaskRecord, TaskRecord,
    VideoGenerationTaskRecord, VideoGenerationTaskViewRecord,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::future::Future;

#[derive(Debug, sqlx::FromRow)]
struct ImageTaskViewRow {
    task_id: String,
    backend_id: String,
    model_id: Option<String>,
    model_path: String,
    prompt: String,
    negative_prompt: Option<String>,
    mode: String,
    width: i64,
    height: i64,
    requested_count: i64,
    reference_image_path: Option<String>,
    primary_image_path: Option<String>,
    artifact_paths: Option<String>,
    request_data: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    task_status: String,
    task_result_data: Option<String>,
    error_msg: Option<String>,
    task_created_at: DateTime<Utc>,
    task_updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct VideoTaskViewRow {
    task_id: String,
    backend_id: String,
    model_id: Option<String>,
    model_path: String,
    prompt: String,
    negative_prompt: Option<String>,
    width: i64,
    height: i64,
    frames: i64,
    fps: f64,
    reference_image_path: Option<String>,
    video_path: Option<String>,
    request_data: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    task_status: String,
    task_result_data: Option<String>,
    error_msg: Option<String>,
    task_created_at: DateTime<Utc>,
    task_updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct AudioTaskViewRow {
    task_id: String,
    backend_id: String,
    model_id: Option<String>,
    source_path: String,
    language: Option<String>,
    prompt: Option<String>,
    detect_language: Option<i64>,
    vad_json: Option<String>,
    decode_json: Option<String>,
    transcript_text: Option<String>,
    request_data: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    task_status: String,
    task_result_data: Option<String>,
    error_msg: Option<String>,
    task_created_at: DateTime<Utc>,
    task_updated_at: DateTime<Utc>,
}

pub trait MediaTaskStore: Send + Sync + 'static {
    fn insert_image_generation_operation(
        &self,
        task: TaskRecord,
        image_task: NewImageGenerationTaskRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn insert_video_generation_operation(
        &self,
        task: TaskRecord,
        video_task: NewVideoGenerationTaskRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn insert_audio_transcription_operation(
        &self,
        task: TaskRecord,
        audio_task: NewAudioTranscriptionTaskRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn update_image_generation_result(
        &self,
        task_id: &str,
        artifact_paths: &[String],
        primary_image_path: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn update_video_generation_result(
        &self,
        task_id: &str,
        video_path: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn update_audio_transcription_result(
        &self,
        task_id: &str,
        transcript_text: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn get_image_generation_task(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<Option<ImageGenerationTaskViewRecord>, sqlx::Error>> + Send;

    fn list_image_generation_tasks(
        &self,
    ) -> impl Future<Output = Result<Vec<ImageGenerationTaskViewRecord>, sqlx::Error>> + Send;

    fn get_video_generation_task(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<Option<VideoGenerationTaskViewRecord>, sqlx::Error>> + Send;

    fn list_video_generation_tasks(
        &self,
    ) -> impl Future<Output = Result<Vec<VideoGenerationTaskViewRecord>, sqlx::Error>> + Send;

    fn get_audio_transcription_task(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<Option<AudioTranscriptionTaskViewRecord>, sqlx::Error>> + Send;

    fn list_audio_transcription_tasks(
        &self,
    ) -> impl Future<Output = Result<Vec<AudioTranscriptionTaskViewRecord>, sqlx::Error>> + Send;
}

impl MediaTaskStore for AnyStore {
    async fn insert_image_generation_operation(
        &self,
        task: TaskRecord,
        image_task: NewImageGenerationTaskRecord,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        super::insert_task_row(&mut tx, &task, task.result_data.as_deref()).await?;
        sqlx::query(
            "INSERT INTO image_generation_tasks \
             (task_id, backend_id, model_id, model_path, prompt, negative_prompt, mode, width, height, requested_count, reference_image_path, primary_image_path, artifact_paths, request_data, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, NULL, ?12, ?13, ?14)",
        )
        .bind(&image_task.task_id)
        .bind(&image_task.backend_id)
        .bind(&image_task.model_id)
        .bind(&image_task.model_path)
        .bind(&image_task.prompt)
        .bind(&image_task.negative_prompt)
        .bind(&image_task.mode)
        .bind(i64::from(image_task.width))
        .bind(i64::from(image_task.height))
        .bind(i64::from(image_task.requested_count))
        .bind(&image_task.reference_image_path)
        .bind(&image_task.request_data)
        .bind(image_task.created_at.to_rfc3339())
        .bind(image_task.updated_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn insert_video_generation_operation(
        &self,
        task: TaskRecord,
        video_task: NewVideoGenerationTaskRecord,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        super::insert_task_row(&mut tx, &task, task.result_data.as_deref()).await?;
        sqlx::query(
            "INSERT INTO video_generation_tasks \
             (task_id, backend_id, model_id, model_path, prompt, negative_prompt, width, height, frames, fps, reference_image_path, video_path, request_data, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, ?12, ?13, ?14)",
        )
        .bind(&video_task.task_id)
        .bind(&video_task.backend_id)
        .bind(&video_task.model_id)
        .bind(&video_task.model_path)
        .bind(&video_task.prompt)
        .bind(&video_task.negative_prompt)
        .bind(i64::from(video_task.width))
        .bind(i64::from(video_task.height))
        .bind(i64::from(video_task.frames))
        .bind(f64::from(video_task.fps))
        .bind(&video_task.reference_image_path)
        .bind(&video_task.request_data)
        .bind(video_task.created_at.to_rfc3339())
        .bind(video_task.updated_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn insert_audio_transcription_operation(
        &self,
        task: TaskRecord,
        audio_task: NewAudioTranscriptionTaskRecord,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        super::insert_task_row(&mut tx, &task, task.result_data.as_deref()).await?;
        sqlx::query(
            "INSERT INTO audio_transcription_tasks \
             (task_id, backend_id, model_id, source_path, language, prompt, detect_language, vad_json, decode_json, transcript_text, request_data, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11, ?12)",
        )
        .bind(&audio_task.task_id)
        .bind(&audio_task.backend_id)
        .bind(&audio_task.model_id)
        .bind(&audio_task.source_path)
        .bind(&audio_task.language)
        .bind(&audio_task.prompt)
        .bind(audio_task.detect_language.map(|value| if value { 1_i64 } else { 0_i64 }))
        .bind(&audio_task.vad_json)
        .bind(&audio_task.decode_json)
        .bind(&audio_task.request_data)
        .bind(audio_task.created_at.to_rfc3339())
        .bind(audio_task.updated_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn update_image_generation_result(
        &self,
        task_id: &str,
        artifact_paths: &[String],
        primary_image_path: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let artifact_paths = serde_json::to_string(artifact_paths).map_err(json_to_sqlx_error)?;
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE image_generation_tasks \
             SET artifact_paths = ?1, primary_image_path = ?2, updated_at = ?3 \
             WHERE task_id = ?4",
        )
        .bind(artifact_paths)
        .bind(primary_image_path)
        .bind(updated_at)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_video_generation_result(
        &self,
        task_id: &str,
        video_path: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE video_generation_tasks \
             SET video_path = ?1, updated_at = ?2 \
             WHERE task_id = ?3",
        )
        .bind(video_path)
        .bind(updated_at)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_audio_transcription_result(
        &self,
        task_id: &str,
        transcript_text: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE audio_transcription_tasks \
             SET transcript_text = ?1, updated_at = ?2 \
             WHERE task_id = ?3",
        )
        .bind(transcript_text)
        .bind(updated_at)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_image_generation_task(
        &self,
        task_id: &str,
    ) -> Result<Option<ImageGenerationTaskViewRecord>, sqlx::Error> {
        let row: Option<ImageTaskViewRow> = sqlx::query_as(IMAGE_TASK_VIEW_QUERY_WITH_ID)
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(image_view_from_row).transpose()
    }

    async fn list_image_generation_tasks(
        &self,
    ) -> Result<Vec<ImageGenerationTaskViewRecord>, sqlx::Error> {
        let rows: Vec<ImageTaskViewRow> =
            sqlx::query_as(IMAGE_TASK_VIEW_QUERY).fetch_all(&self.pool).await?;
        rows.into_iter().map(image_view_from_row).collect()
    }

    async fn get_video_generation_task(
        &self,
        task_id: &str,
    ) -> Result<Option<VideoGenerationTaskViewRecord>, sqlx::Error> {
        let row: Option<VideoTaskViewRow> = sqlx::query_as(VIDEO_TASK_VIEW_QUERY_WITH_ID)
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(video_view_from_row).transpose()
    }

    async fn list_video_generation_tasks(
        &self,
    ) -> Result<Vec<VideoGenerationTaskViewRecord>, sqlx::Error> {
        let rows: Vec<VideoTaskViewRow> =
            sqlx::query_as(VIDEO_TASK_VIEW_QUERY).fetch_all(&self.pool).await?;
        rows.into_iter().map(video_view_from_row).collect()
    }

    async fn get_audio_transcription_task(
        &self,
        task_id: &str,
    ) -> Result<Option<AudioTranscriptionTaskViewRecord>, sqlx::Error> {
        let row: Option<AudioTaskViewRow> = sqlx::query_as(AUDIO_TASK_VIEW_QUERY_WITH_ID)
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(audio_view_from_row).transpose()
    }

    async fn list_audio_transcription_tasks(
        &self,
    ) -> Result<Vec<AudioTranscriptionTaskViewRecord>, sqlx::Error> {
        let rows: Vec<AudioTaskViewRow> =
            sqlx::query_as(AUDIO_TASK_VIEW_QUERY).fetch_all(&self.pool).await?;
        rows.into_iter().map(audio_view_from_row).collect()
    }
}

const IMAGE_TASK_VIEW_QUERY: &str = "SELECT i.task_id, i.backend_id, i.model_id, i.model_path, i.prompt, i.negative_prompt, i.mode, i.width, i.height, i.requested_count, i.reference_image_path, i.primary_image_path, i.artifact_paths, i.request_data, i.created_at, i.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM image_generation_tasks i JOIN tasks t ON t.id = i.task_id ORDER BY t.created_at DESC";
const IMAGE_TASK_VIEW_QUERY_WITH_ID: &str = "SELECT i.task_id, i.backend_id, i.model_id, i.model_path, i.prompt, i.negative_prompt, i.mode, i.width, i.height, i.requested_count, i.reference_image_path, i.primary_image_path, i.artifact_paths, i.request_data, i.created_at, i.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM image_generation_tasks i JOIN tasks t ON t.id = i.task_id WHERE i.task_id = ?1";

const VIDEO_TASK_VIEW_QUERY: &str = "SELECT v.task_id, v.backend_id, v.model_id, v.model_path, v.prompt, v.negative_prompt, v.width, v.height, v.frames, v.fps, v.reference_image_path, v.video_path, v.request_data, v.created_at, v.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM video_generation_tasks v JOIN tasks t ON t.id = v.task_id ORDER BY t.created_at DESC";
const VIDEO_TASK_VIEW_QUERY_WITH_ID: &str = "SELECT v.task_id, v.backend_id, v.model_id, v.model_path, v.prompt, v.negative_prompt, v.width, v.height, v.frames, v.fps, v.reference_image_path, v.video_path, v.request_data, v.created_at, v.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM video_generation_tasks v JOIN tasks t ON t.id = v.task_id WHERE v.task_id = ?1";

const AUDIO_TASK_VIEW_QUERY: &str = "SELECT a.task_id, a.backend_id, a.model_id, a.source_path, a.language, a.prompt, a.detect_language, a.vad_json, a.decode_json, a.transcript_text, a.request_data, a.created_at, a.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM audio_transcription_tasks a JOIN tasks t ON t.id = a.task_id ORDER BY t.created_at DESC";
const AUDIO_TASK_VIEW_QUERY_WITH_ID: &str = "SELECT a.task_id, a.backend_id, a.model_id, a.source_path, a.language, a.prompt, a.detect_language, a.vad_json, a.decode_json, a.transcript_text, a.request_data, a.created_at, a.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM audio_transcription_tasks a JOIN tasks t ON t.id = a.task_id WHERE a.task_id = ?1";

fn image_view_from_row(
    row: ImageTaskViewRow,
) -> Result<ImageGenerationTaskViewRecord, sqlx::Error> {
    let artifact_paths =
        decode_string_array(row.artifact_paths.as_deref(), &row.task_id, "artifact_paths")?;
    let result_data =
        image_result_data(&row.task_id, row.primary_image_path.clone(), artifact_paths.clone());
    Ok(ImageGenerationTaskViewRecord {
        task: ImageGenerationTaskRecord {
            task_id: row.task_id.clone(),
            backend_id: row.backend_id,
            model_id: row.model_id,
            model_path: row.model_path,
            prompt: row.prompt,
            negative_prompt: row.negative_prompt,
            mode: row.mode,
            width: checked_u32(row.width, &row.task_id, "width")?,
            height: checked_u32(row.height, &row.task_id, "height")?,
            requested_count: checked_u32(row.requested_count, &row.task_id, "requested_count")?,
            reference_image_path: row.reference_image_path,
            primary_image_path: row.primary_image_path,
            artifact_paths,
            request_data: row.request_data,
            result_data,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
        state: media_state_from_task(
            row.task_status,
            row.task_result_data,
            row.error_msg,
            row.task_created_at,
            row.task_updated_at,
        ),
    })
}

fn video_view_from_row(
    row: VideoTaskViewRow,
) -> Result<VideoGenerationTaskViewRecord, sqlx::Error> {
    let result_data = video_result_data(&row.task_id, row.video_path.clone());
    Ok(VideoGenerationTaskViewRecord {
        task: VideoGenerationTaskRecord {
            task_id: row.task_id.clone(),
            backend_id: row.backend_id,
            model_id: row.model_id,
            model_path: row.model_path,
            prompt: row.prompt,
            negative_prompt: row.negative_prompt,
            width: checked_u32(row.width, &row.task_id, "width")?,
            height: checked_u32(row.height, &row.task_id, "height")?,
            frames: checked_i32(row.frames, &row.task_id, "frames")?,
            fps: row.fps,
            reference_image_path: row.reference_image_path,
            video_path: row.video_path,
            request_data: row.request_data,
            result_data,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
        state: media_state_from_task(
            row.task_status,
            row.task_result_data,
            row.error_msg,
            row.task_created_at,
            row.task_updated_at,
        ),
    })
}

fn audio_view_from_row(
    row: AudioTaskViewRow,
) -> Result<AudioTranscriptionTaskViewRecord, sqlx::Error> {
    let task_result_data = row.task_result_data;
    let decoded_task_payload = super::task::decode_task_payload(task_result_data.clone());
    let result_data = audio_result_data(
        &row.task_id,
        row.transcript_text.clone(),
        decoded_task_payload.as_deref(),
    );
    Ok(AudioTranscriptionTaskViewRecord {
        task: AudioTranscriptionTaskRecord {
            task_id: row.task_id.clone(),
            backend_id: row.backend_id,
            model_id: row.model_id,
            source_path: row.source_path,
            language: row.language,
            prompt: row.prompt,
            detect_language: row
                .detect_language
                .map(|value| checked_bool(value, &row.task_id, "detect_language"))
                .transpose()?,
            vad_json: row.vad_json,
            decode_json: row.decode_json,
            transcript_text: row.transcript_text,
            request_data: row.request_data,
            result_data,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
        state: media_state_from_task(
            row.task_status,
            task_result_data,
            row.error_msg,
            row.task_created_at,
            row.task_updated_at,
        ),
    })
}

fn media_state_from_task(
    status: String,
    result_data: Option<String>,
    error_msg: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> MediaTaskState {
    let result_data = super::task::decode_task_payload(result_data);
    MediaTaskState {
        status: TaskStatus::from_stored(&status, "media task repository"),
        progress: task_progress_from_payload(result_data.as_deref()),
        error_msg,
        task_created_at: created_at,
        task_updated_at: updated_at,
    }
}

fn image_result_data(
    task_id: &str,
    primary_image_path: Option<String>,
    artifact_paths: Vec<String>,
) -> Option<String> {
    if primary_image_path.is_none() && artifact_paths.is_empty() {
        return None;
    }
    serialize_media_result_data(
        task_id,
        "image result_data",
        &ImageGenerationResultData { primary_image_path, artifact_paths },
    )
}

fn video_result_data(task_id: &str, video_path: Option<String>) -> Option<String> {
    video_path.as_ref()?;
    serialize_media_result_data(
        task_id,
        "video result_data",
        &VideoGenerationResultData { video_path },
    )
}

fn audio_result_data(
    task_id: &str,
    transcript_text: Option<String>,
    decoded_task_payload: Option<&str>,
) -> Option<String> {
    if let Some(payload) = decoded_task_payload {
        match serde_json::from_str::<TaskResult>(payload) {
            Ok(task_result) => {
                if task_result.text.is_some() || task_result.segments.is_some() {
                    return serialize_media_result_data(
                        task_id,
                        "audio result_data",
                        &AudioTranscriptionResultData {
                            text: task_result.text.unwrap_or_default(),
                            segments: task_result.segments.unwrap_or_default(),
                        },
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    task_id,
                    error = %error,
                    "failed to derive audio result_data from task payload"
                );
            }
        }
    }
    transcript_text.and_then(|text| {
        serialize_media_result_data(
            task_id,
            "audio result_data",
            &AudioTranscriptionResultData { text, segments: Vec::new() },
        )
    })
}

fn serialize_media_result_data<T: Serialize>(
    task_id: &str,
    field: &'static str,
    value: &T,
) -> Option<String> {
    match serde_json::to_string(value) {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(
                task_id,
                field,
                error = %error,
                "failed to serialize media artifact payload"
            );
            None
        }
    }
}

fn decode_string_array(
    raw: Option<&str>,
    task_id: &str,
    field: &'static str,
) -> Result<Vec<String>, sqlx::Error> {
    let Some(value) = raw else {
        return Ok(Vec::new());
    };
    serde_json::from_str::<Vec<String>>(value).map_err(|error| {
        tracing::warn!(
            task_id,
            field,
            error = %error,
            "failed to decode stored string array"
        );
        json_to_sqlx_error(error)
    })
}

fn checked_u32(value: i64, task_id: &str, field: &'static str) -> Result<u32, sqlx::Error> {
    u32::try_from(value).map_err(|error| {
        tracing::warn!(
            task_id,
            field,
            value,
            error = %error,
            "stored media numeric field is outside u32 range"
        );
        sqlx::Error::Decode(Box::new(error))
    })
}

fn checked_i32(value: i64, task_id: &str, field: &'static str) -> Result<i32, sqlx::Error> {
    i32::try_from(value).map_err(|error| {
        tracing::warn!(
            task_id,
            field,
            value,
            error = %error,
            "stored media numeric field is outside i32 range"
        );
        sqlx::Error::Decode(Box::new(error))
    })
}

fn checked_bool(value: i64, task_id: &str, field: &'static str) -> Result<bool, sqlx::Error> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        value => {
            tracing::warn!(
                task_id,
                field,
                value,
                "stored media boolean field is outside 0/1 range"
            );
            Err(sqlx::Error::Decode(format!("invalid {field} boolean value: {value}").into()))
        }
    }
}

fn json_to_sqlx_error(error: serde_json::Error) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{
        AUDIO_TRANSCRIPTION_TASK_TYPE, IMAGE_GENERATION_TASK_TYPE, VIDEO_GENERATION_TASK_TYPE,
    };
    use crate::test_support::migrated_test_store;

    #[tokio::test]
    async fn image_generation_task_round_trips_result_fields() {
        let store = migrated_test_store().await;
        let task = task_record("image-task", IMAGE_GENERATION_TASK_TYPE);
        let created_at = timestamp();

        store
            .insert_image_generation_operation(
                task,
                NewImageGenerationTaskRecord {
                    task_id: "image-task".to_owned(),
                    backend_id: "ggml.diffusion".to_owned(),
                    model_id: Some("model-1".to_owned()),
                    model_path: "models/image.safetensors".to_owned(),
                    prompt: "a quiet test".to_owned(),
                    negative_prompt: Some("noise".to_owned()),
                    mode: "txt2img".to_owned(),
                    width: 512,
                    height: 768,
                    requested_count: 2,
                    reference_image_path: Some("input.png".to_owned()),
                    request_data: r#"{"prompt":"a quiet test"}"#.to_owned(),
                    created_at,
                    updated_at: created_at,
                },
            )
            .await
            .expect("insert image operation");

        store
            .update_image_generation_result(
                "image-task",
                &["first.png".to_owned(), "second.png".to_owned()],
                Some("first.png"),
            )
            .await
            .expect("update image result");

        let view = store
            .get_image_generation_task("image-task")
            .await
            .expect("get image task")
            .expect("image task exists");
        assert_eq!(view.task.backend_id, "ggml.diffusion");
        assert_eq!(view.task.model_id.as_deref(), Some("model-1"));
        assert_eq!(view.task.width, 512);
        assert_eq!(view.task.height, 768);
        assert_eq!(view.task.requested_count, 2);
        assert_eq!(view.task.primary_image_path.as_deref(), Some("first.png"));
        assert_eq!(view.task.artifact_paths, vec!["first.png", "second.png"]);
        assert_eq!(
            view.task.result_data.as_deref(),
            Some(
                r#"{"primary_image_path":"first.png","artifact_paths":["first.png","second.png"]}"#
            )
        );
        assert_eq!(view.state.status, TaskStatus::Pending);

        let list = store.list_image_generation_tasks().await.expect("list image tasks");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].task.task_id, "image-task");
    }

    #[tokio::test]
    async fn video_generation_task_round_trips_result_fields() {
        let store = migrated_test_store().await;
        let task = task_record("video-task", VIDEO_GENERATION_TASK_TYPE);
        let created_at = timestamp();

        store
            .insert_video_generation_operation(
                task,
                NewVideoGenerationTaskRecord {
                    task_id: "video-task".to_owned(),
                    backend_id: "ggml.diffusion".to_owned(),
                    model_id: Some("model-2".to_owned()),
                    model_path: "models/video.safetensors".to_owned(),
                    prompt: "a moving test".to_owned(),
                    negative_prompt: None,
                    width: 640,
                    height: 360,
                    frames: 24,
                    fps: 12.5,
                    reference_image_path: Some("frame.png".to_owned()),
                    request_data: r#"{"prompt":"a moving test"}"#.to_owned(),
                    created_at,
                    updated_at: created_at,
                },
            )
            .await
            .expect("insert video operation");

        store
            .update_video_generation_result("video-task", Some("video.mp4"))
            .await
            .expect("update video result");

        let view = store
            .get_video_generation_task("video-task")
            .await
            .expect("get video task")
            .expect("video task exists");
        assert_eq!(view.task.backend_id, "ggml.diffusion");
        assert_eq!(view.task.model_id.as_deref(), Some("model-2"));
        assert_eq!(view.task.frames, 24);
        assert_eq!(view.task.fps, 12.5);
        assert_eq!(view.task.video_path.as_deref(), Some("video.mp4"));
        assert_eq!(view.task.result_data.as_deref(), Some(r#"{"video_path":"video.mp4"}"#));
        assert_eq!(view.state.status, TaskStatus::Pending);

        let list = store.list_video_generation_tasks().await.expect("list video tasks");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].task.task_id, "video-task");
    }

    #[tokio::test]
    async fn audio_transcription_task_round_trips_result_fields() {
        let store = migrated_test_store().await;
        let task = task_record("audio-task", AUDIO_TRANSCRIPTION_TASK_TYPE);
        let created_at = timestamp();

        store
            .insert_audio_transcription_operation(
                task,
                NewAudioTranscriptionTaskRecord {
                    task_id: "audio-task".to_owned(),
                    backend_id: "ggml.whisper".to_owned(),
                    model_id: Some("model-3".to_owned()),
                    source_path: "audio.wav".to_owned(),
                    language: Some("en".to_owned()),
                    prompt: Some("domain words".to_owned()),
                    detect_language: Some(true),
                    vad_json: Some(r#"{"enabled":true}"#.to_owned()),
                    decode_json: Some(r#"{"temperature":0.2}"#.to_owned()),
                    request_data: r#"{"source_path":"audio.wav"}"#.to_owned(),
                    created_at,
                    updated_at: created_at,
                },
            )
            .await
            .expect("insert audio operation");

        store
            .update_audio_transcription_result("audio-task", Some("hello world"))
            .await
            .expect("update audio result");

        let view = store
            .get_audio_transcription_task("audio-task")
            .await
            .expect("get audio task")
            .expect("audio task exists");
        assert_eq!(view.task.backend_id, "ggml.whisper");
        assert_eq!(view.task.model_id.as_deref(), Some("model-3"));
        assert_eq!(view.task.language.as_deref(), Some("en"));
        assert_eq!(view.task.prompt.as_deref(), Some("domain words"));
        assert_eq!(view.task.detect_language, Some(true));
        assert_eq!(view.task.transcript_text.as_deref(), Some("hello world"));
        assert_eq!(
            view.task.result_data.as_deref(),
            Some(r#"{"text":"hello world","segments":[]}"#)
        );
        assert_eq!(view.state.status, TaskStatus::Pending);

        let list = store.list_audio_transcription_tasks().await.expect("list audio tasks");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].task.task_id, "audio-task");
    }

    fn task_record(id: &str, task_type: &str) -> TaskRecord {
        let now = timestamp();
        TaskRecord {
            id: id.to_owned(),
            task_type: task_type.to_owned(),
            status: TaskStatus::Pending,
            model_id: None,
            input_data: None,
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-17T00:00:00Z")
            .expect("test timestamp")
            .with_timezone(&Utc)
    }
}
