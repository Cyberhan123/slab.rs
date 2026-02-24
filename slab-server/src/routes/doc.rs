use crate::routes::admin;
use crate::routes::v1;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(info(
    title = "slab-server",
    description = "slab-server API",
    version = "0.0.1",
    contact(name = "slab-rs", url = "https://github.com/Cyberhan123/slab.rs")
))]
pub struct ApiDoc;

pub fn get_docs() -> utoipa::openapi::OpenApi {
    let mut root = ApiDoc::openapi();
    root.merge(v1::api_docs());
    root.merge(admin::api_docs());
    root
}
