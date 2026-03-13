use base64::Engine as _;

use crate::domain::models::TaskResult;

pub fn payload_to_task_result(task_type: &str, payload: &slab_core::Payload) -> TaskResult {
    match payload {
        slab_core::Payload::Bytes(bytes) => {
            if task_type == "image" {
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes.as_ref());
                let uri = format!("data:image/png;base64,{encoded}");
                TaskResult {
                    image: Some(uri.clone()),
                    images: Some(vec![uri]),
                    video_path: None,
                    text: None,
                }
            } else {
                TaskResult {
                    image: None,
                    images: None,
                    video_path: None,
                    text: Some(String::from_utf8_lossy(bytes).to_string()),
                }
            }
        }
        slab_core::Payload::Text(text) => TaskResult {
            image: None,
            images: None,
            video_path: None,
            text: Some(text.to_string()),
        },
        slab_core::Payload::Json(value) => {
            let image = value
                .get("image")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let images = value.get("images").and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
            });
            let video_path = value
                .get("video_path")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let text = value
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .or_else(|| {
                    if image.is_none() && video_path.is_none() {
                        Some(value.to_string())
                    } else {
                        None
                    }
                });

            TaskResult {
                image,
                images,
                video_path,
                text,
            }
        }
        slab_core::Payload::None => TaskResult {
            image: None,
            images: None,
            video_path: None,
            text: None,
        },
        slab_core::Payload::F32(values) => TaskResult {
            image: None,
            images: None,
            video_path: None,
            text: Some(format!("f32[{}]", values.len())),
        },
        slab_core::Payload::Any(_) => TaskResult {
            image: None,
            images: None,
            video_path: None,
            text: Some("unsupported payload".to_owned()),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use base64::Engine as _;

    use super::payload_to_task_result;

    #[test]
    fn image_payload_maps_to_data_uri() {
        let payload = slab_core::Payload::Bytes(Arc::<[u8]>::from(&b"\x89PNG\r\n\x1a\n"[..]));
        let result = payload_to_task_result("image", &payload);

        let uri = result.image.expect("image must exist");
        assert!(uri.starts_with("data:image/png;base64,"));
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(uri.trim_start_matches("data:image/png;base64,"))
            .expect("must decode");
        assert_eq!(decoded, b"\x89PNG\r\n\x1a\n");
    }
}
