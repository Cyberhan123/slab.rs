use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::subtitles::schema::{
    RenderSubtitleRequest, RenderSubtitleResponse, SubtitleEntryRequest, SubtitleFormatRequest,
    SubtitleVariantRequest,
};
use crate::api::validation::ValidatedJson;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::SubtitleService;

#[derive(OpenApi)]
#[openapi(
    paths(render_subtitle),
    components(schemas(
        RenderSubtitleRequest,
        RenderSubtitleResponse,
        SubtitleEntryRequest,
        SubtitleFormatRequest,
        SubtitleVariantRequest
    ))
)]
pub struct SubtitleApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/subtitles/render", post(render_subtitle))
}

#[utoipa::path(
    post,
    path = "/v1/subtitles/render",
    tag = "subtitles",
    request_body = RenderSubtitleRequest,
    responses(
        (status = 200, description = "Subtitle file rendered", body = RenderSubtitleResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn render_subtitle(
    State(service): State<SubtitleService>,
    ValidatedJson(req): ValidatedJson<RenderSubtitleRequest>,
) -> Result<Json<RenderSubtitleResponse>, ServerError> {
    let result = service.render(req.into()).await?;
    Ok(Json(RenderSubtitleResponse {
        output_path: result.output_path,
        format: result.format,
        entry_count: result.entry_count,
    }))
}
