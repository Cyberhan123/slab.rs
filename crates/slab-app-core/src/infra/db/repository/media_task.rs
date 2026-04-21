use super::AnyStore;
use crate::domain::models::{TaskProgress, TaskStatus};
use crate::infra::db::entities::{
    AudioTranscriptionTaskRecord, AudioTranscriptionTaskViewRecord, ImageGenerationTaskRecord,
    ImageGenerationTaskViewRecord, MediaTaskState, NewAudioTranscriptionTaskRecord,
    NewImageGenerationTaskRecord, NewVideoGenerationTaskRecord, TaskRecord,
    VideoGenerationTaskRecord, VideoGenerationTaskViewRecord,
};
use chrono::Utc;
use serde_json::Value;
use std::future::Future;
use std::str::FromStr;

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
    result_data: Option<String>,
    created_at: String,
    updated_at: String,
    task_status: String,
    task_result_data: Option<String>,
    error_msg: Option<String>,
    task_created_at: String,
    task_updated_at: String,
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
    result_data: Option<String>,
    created_at: String,
    updated_at: String,
    task_status: String,
    task_result_data: Option<String>,
    error_msg: Option<String>,
    task_created_at: String,
    task_updated_at: String,
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
    result_data: Option<String>,
    created_at: String,
    updated_at: String,
    task_status: String,
    task_result_data: Option<String>,
    error_msg: Option<String>,
    task_created_at: String,
    task_updated_at: String,
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
        result_data: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn update_video_generation_result(
        &self,
        task_id: &str,
        video_path: Option<&str>,
        result_data: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn update_audio_transcription_result(
        &self,
        task_id: &str,
        transcript_text: Option<&str>,
        result_data: Option<&str>,
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
        insert_task_in_tx(&mut tx, &task).await?;
        sqlx::query(
            "INSERT INTO image_generation_tasks \
             (task_id, backend_id, model_id, model_path, prompt, negative_prompt, mode, width, height, requested_count, reference_image_path, primary_image_path, artifact_paths, request_data, result_data, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, NULL, ?12, NULL, ?13, ?14)",
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
        insert_task_in_tx(&mut tx, &task).await?;
        sqlx::query(
            "INSERT INTO video_generation_tasks \
             (task_id, backend_id, model_id, model_path, prompt, negative_prompt, width, height, frames, fps, reference_image_path, video_path, request_data, result_data, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, ?12, NULL, ?13, ?14)",
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
        insert_task_in_tx(&mut tx, &task).await?;
        sqlx::query(
            "INSERT INTO audio_transcription_tasks \
             (task_id, backend_id, model_id, source_path, language, prompt, detect_language, vad_json, decode_json, transcript_text, request_data, result_data, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, NULL, ?11, ?12)",
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
        result_data: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let artifact_paths =
            serde_json::to_string(artifact_paths).unwrap_or_else(|_| "[]".to_owned());
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE image_generation_tasks \
             SET artifact_paths = ?1, primary_image_path = ?2, result_data = ?3, updated_at = ?4 \
             WHERE task_id = ?5",
        )
        .bind(artifact_paths)
        .bind(primary_image_path)
        .bind(result_data)
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
        result_data: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE video_generation_tasks \
             SET video_path = ?1, result_data = ?2, updated_at = ?3 \
             WHERE task_id = ?4",
        )
        .bind(video_path)
        .bind(result_data)
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
        result_data: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE audio_transcription_tasks \
             SET transcript_text = ?1, result_data = ?2, updated_at = ?3 \
             WHERE task_id = ?4",
        )
        .bind(transcript_text)
        .bind(result_data)
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
        Ok(row.map(image_view_from_row))
    }

    async fn list_image_generation_tasks(
        &self,
    ) -> Result<Vec<ImageGenerationTaskViewRecord>, sqlx::Error> {
        let rows: Vec<ImageTaskViewRow> =
            sqlx::query_as(IMAGE_TASK_VIEW_QUERY).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(image_view_from_row).collect())
    }

    async fn get_video_generation_task(
        &self,
        task_id: &str,
    ) -> Result<Option<VideoGenerationTaskViewRecord>, sqlx::Error> {
        let row: Option<VideoTaskViewRow> = sqlx::query_as(VIDEO_TASK_VIEW_QUERY_WITH_ID)
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(video_view_from_row))
    }

    async fn list_video_generation_tasks(
        &self,
    ) -> Result<Vec<VideoGenerationTaskViewRecord>, sqlx::Error> {
        let rows: Vec<VideoTaskViewRow> =
            sqlx::query_as(VIDEO_TASK_VIEW_QUERY).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(video_view_from_row).collect())
    }

    async fn get_audio_transcription_task(
        &self,
        task_id: &str,
    ) -> Result<Option<AudioTranscriptionTaskViewRecord>, sqlx::Error> {
        let row: Option<AudioTaskViewRow> = sqlx::query_as(AUDIO_TASK_VIEW_QUERY_WITH_ID)
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(audio_view_from_row))
    }

    async fn list_audio_transcription_tasks(
        &self,
    ) -> Result<Vec<AudioTranscriptionTaskViewRecord>, sqlx::Error> {
        let rows: Vec<AudioTaskViewRow> =
            sqlx::query_as(AUDIO_TASK_VIEW_QUERY).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(audio_view_from_row).collect())
    }
}

const IMAGE_TASK_VIEW_QUERY: &str = "SELECT i.task_id, i.backend_id, i.model_id, i.model_path, i.prompt, i.negative_prompt, i.mode, i.width, i.height, i.requested_count, i.reference_image_path, i.primary_image_path, i.artifact_paths, i.request_data, i.result_data, i.created_at, i.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM image_generation_tasks i JOIN tasks t ON t.id = i.task_id ORDER BY t.created_at DESC";
const IMAGE_TASK_VIEW_QUERY_WITH_ID: &str = "SELECT i.task_id, i.backend_id, i.model_id, i.model_path, i.prompt, i.negative_prompt, i.mode, i.width, i.height, i.requested_count, i.reference_image_path, i.primary_image_path, i.artifact_paths, i.request_data, i.result_data, i.created_at, i.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM image_generation_tasks i JOIN tasks t ON t.id = i.task_id WHERE i.task_id = ?1";

const VIDEO_TASK_VIEW_QUERY: &str = "SELECT v.task_id, v.backend_id, v.model_id, v.model_path, v.prompt, v.negative_prompt, v.width, v.height, v.frames, v.fps, v.reference_image_path, v.video_path, v.request_data, v.result_data, v.created_at, v.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM video_generation_tasks v JOIN tasks t ON t.id = v.task_id ORDER BY t.created_at DESC";
const VIDEO_TASK_VIEW_QUERY_WITH_ID: &str = "SELECT v.task_id, v.backend_id, v.model_id, v.model_path, v.prompt, v.negative_prompt, v.width, v.height, v.frames, v.fps, v.reference_image_path, v.video_path, v.request_data, v.result_data, v.created_at, v.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM video_generation_tasks v JOIN tasks t ON t.id = v.task_id WHERE v.task_id = ?1";

const AUDIO_TASK_VIEW_QUERY: &str = "SELECT a.task_id, a.backend_id, a.model_id, a.source_path, a.language, a.prompt, a.detect_language, a.vad_json, a.decode_json, a.transcript_text, a.request_data, a.result_data, a.created_at, a.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM audio_transcription_tasks a JOIN tasks t ON t.id = a.task_id ORDER BY t.created_at DESC";
const AUDIO_TASK_VIEW_QUERY_WITH_ID: &str = "SELECT a.task_id, a.backend_id, a.model_id, a.source_path, a.language, a.prompt, a.detect_language, a.vad_json, a.decode_json, a.transcript_text, a.request_data, a.result_data, a.created_at, a.updated_at, t.status AS task_status, t.result_data AS task_result_data, t.error_msg, t.created_at AS task_created_at, t.updated_at AS task_updated_at FROM audio_transcription_tasks a JOIN tasks t ON t.id = a.task_id WHERE a.task_id = ?1";

async fn insert_task_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    task: &TaskRecord,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tasks (id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )
    .bind(&task.id)
    .bind(&task.task_type)
    .bind(task.status.as_str())
    .bind(&task.model_id)
    .bind(&task.input_data)
    .bind(&task.result_data)
    .bind(&task.error_msg)
    .bind(task.core_task_id)
    .bind(task.created_at.to_rfc3339())
    .bind(task.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn image_view_from_row(row: ImageTaskViewRow) -> ImageGenerationTaskViewRecord {
    ImageGenerationTaskViewRecord {
        task: ImageGenerationTaskRecord {
            task_id: row.task_id,
            backend_id: row.backend_id,
            model_id: row.model_id,
            model_path: row.model_path,
            prompt: row.prompt,
            negative_prompt: row.negative_prompt,
            mode: row.mode,
            width: to_u32(row.width),
            height: to_u32(row.height),
            requested_count: to_u32(row.requested_count),
            reference_image_path: row.reference_image_path,
            primary_image_path: row.primary_image_path,
            artifact_paths: decode_string_array(row.artifact_paths.as_deref()),
            request_data: row.request_data,
            result_data: row.result_data,
            created_at: parse_rfc3339_or_now(row.created_at, "image.created_at"),
            updated_at: parse_rfc3339_or_now(row.updated_at, "image.updated_at"),
        },
        state: media_state_from_task(
            row.task_status,
            row.task_result_data,
            row.error_msg,
            row.task_created_at,
            row.task_updated_at,
        ),
    }
}

fn video_view_from_row(row: VideoTaskViewRow) -> VideoGenerationTaskViewRecord {
    VideoGenerationTaskViewRecord {
        task: VideoGenerationTaskRecord {
            task_id: row.task_id,
            backend_id: row.backend_id,
            model_id: row.model_id,
            model_path: row.model_path,
            prompt: row.prompt,
            negative_prompt: row.negative_prompt,
            width: to_u32(row.width),
            height: to_u32(row.height),
            frames: row.frames.try_into().unwrap_or_default(),
            fps: row.fps as f32,
            reference_image_path: row.reference_image_path,
            video_path: row.video_path,
            request_data: row.request_data,
            result_data: row.result_data,
            created_at: parse_rfc3339_or_now(row.created_at, "video.created_at"),
            updated_at: parse_rfc3339_or_now(row.updated_at, "video.updated_at"),
        },
        state: media_state_from_task(
            row.task_status,
            row.task_result_data,
            row.error_msg,
            row.task_created_at,
            row.task_updated_at,
        ),
    }
}

fn audio_view_from_row(row: AudioTaskViewRow) -> AudioTranscriptionTaskViewRecord {
    AudioTranscriptionTaskViewRecord {
        task: AudioTranscriptionTaskRecord {
            task_id: row.task_id,
            backend_id: row.backend_id,
            model_id: row.model_id,
            source_path: row.source_path,
            language: row.language,
            prompt: row.prompt,
            detect_language: row.detect_language.map(|value| value != 0),
            vad_json: row.vad_json,
            decode_json: row.decode_json,
            transcript_text: row.transcript_text,
            request_data: row.request_data,
            result_data: row.result_data,
            created_at: parse_rfc3339_or_now(row.created_at, "audio.created_at"),
            updated_at: parse_rfc3339_or_now(row.updated_at, "audio.updated_at"),
        },
        state: media_state_from_task(
            row.task_status,
            row.task_result_data,
            row.error_msg,
            row.task_created_at,
            row.task_updated_at,
        ),
    }
}

fn media_state_from_task(
    status: String,
    result_data: Option<String>,
    error_msg: Option<String>,
    created_at: String,
    updated_at: String,
) -> MediaTaskState {
    MediaTaskState {
        status: decode_task_status(&status),
        progress: task_progress_from_payload(result_data.as_deref()),
        error_msg,
        task_created_at: parse_rfc3339_or_now(created_at, "task.created_at"),
        task_updated_at: parse_rfc3339_or_now(updated_at, "task.updated_at"),
    }
}

fn parse_rfc3339_or_now(raw: String, field: &'static str) -> chrono::DateTime<Utc> {
    raw.parse().unwrap_or_else(|error: chrono::ParseError| {
        tracing::warn!(raw = %raw, error = %error, field, "failed to parse media task timestamp; using now");
        Utc::now()
    })
}

fn decode_task_status(raw: &str) -> TaskStatus {
    TaskStatus::from_str(raw).unwrap_or_else(|_| {
        tracing::warn!(status = %raw, "unknown media task status stored in repository; defaulting to failed");
        TaskStatus::Failed
    })
}

fn task_progress_from_payload(raw: Option<&str>) -> Option<TaskProgress> {
    let raw = raw?;
    let payload: Value = serde_json::from_str(raw).ok()?;
    serde_json::from_value(payload.get("progress")?.clone()).ok()
}

fn decode_string_array(raw: Option<&str>) -> Vec<String> {
    raw.and_then(|value| serde_json::from_str::<Vec<String>>(value).ok()).unwrap_or_default()
}

fn to_u32(value: i64) -> u32 {
    value.try_into().unwrap_or_default()
}
