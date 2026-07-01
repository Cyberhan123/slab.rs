//! Independent a2u surface windows (TC-FE-06 / INFRA-11).
//!
//! Each a2u surface (workspace / image / review / plugin / hub) can be opened in
//! its own OS window. The window **label** encodes the surface identity
//! (`a2u-<surface>-<id>`); the frontend reads its own window label on load to
//! decide which surface to render, so the caller identity is derived from the
//! label (not a plugin-supplied payload field — AGENTS.md boundary). The window
//! loads the main SPA (`index.html`) and self-routes from the label, avoiding
//! fragile URL hash/path wiring.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::utils::config::WebviewUrl;
use tauri::{AppHandle, Manager, State};

const SURFACE_WINDOW_PREFIX: &str = "a2u-";

/// One a2u surface kind that can be opened in a dedicated window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceKind {
    Workspace,
    Image,
    Review,
    Plugin,
    Hub,
}

impl SurfaceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Image => "image",
            Self::Review => "review",
            Self::Plugin => "plugin",
            Self::Hub => "hub",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceWindowRequest {
    pub surface: SurfaceKind,
    /// Stable id scoping this surface instance (so reopening focuses instead of
    /// spawning a duplicate).
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceWindowResponse {
    pub label: String,
    /// `false` when an existing window for this surface was focused instead.
    pub opened: bool,
}

#[derive(Default)]
pub struct SurfaceWindowManager {
    label_to_surface: Mutex<HashMap<String, String>>,
}

impl SurfaceWindowManager {
    fn track(&self, label: &str, surface: &str) -> Result<(), String> {
        let mut guard = self
            .label_to_surface
            .lock()
            .map_err(|_| "failed to lock surface window manager".to_string())?;
        guard.insert(label.to_string(), surface.to_string());
        Ok(())
    }

    fn forget(&self, label: &str) -> Result<Option<String>, String> {
        let mut guard = self
            .label_to_surface
            .lock()
            .map_err(|_| "failed to lock surface window manager".to_string())?;
        Ok(guard.remove(label))
    }

    fn labels(&self) -> Result<Vec<String>, String> {
        let guard = self
            .label_to_surface
            .lock()
            .map_err(|_| "failed to lock surface window manager".to_string())?;
        Ok(guard.keys().cloned().collect())
    }
}

/// Open (or focus) a dedicated OS window for an a2u surface.
#[tauri::command]
pub fn open_surface_window(
    app_handle: AppHandle,
    manager: State<'_, SurfaceWindowManager>,
    request: SurfaceWindowRequest,
) -> Result<SurfaceWindowResponse, String> {
    let surface = request.surface.as_str();
    let label = surface_window_label(surface, &request.id);

    // Reuse + focus an existing window for this surface instance.
    if let Some(existing) = app_handle.get_webview_window(&label) {
        existing.set_focus().map_err(|e| format!("failed to focus surface window: {e}"))?;
        manager.track(&label, surface)?;
        return Ok(SurfaceWindowResponse { label, opened: false });
    }

    // Load the SPA; the frontend self-routes from the window label on load.
    let url = WebviewUrl::App("index.html".into());
    tauri::webview::WebviewWindowBuilder::new(&app_handle, label.clone(), url)
        .title(format!("Slab — {surface}"))
        .inner_size(1024.0, 720.0)
        .build()
        .map_err(|e| format!("failed to build surface window: {e}"))?;

    manager.track(&label, surface)?;
    Ok(SurfaceWindowResponse { label, opened: true })
}

/// Close a surface window by label.
#[tauri::command]
pub fn close_surface_window(
    app_handle: AppHandle,
    manager: State<'_, SurfaceWindowManager>,
    label: String,
) -> Result<(), String> {
    manager.forget(&label)?;
    if let Some(window) = app_handle.get_webview_window(&label) {
        window.close().map_err(|e| format!("failed to close surface window: {e}"))?;
    }
    Ok(())
}

/// Focus a surface window by label.
#[tauri::command]
pub fn focus_surface_window(app_handle: AppHandle, label: String) -> Result<(), String> {
    let window = app_handle
        .get_webview_window(&label)
        .ok_or_else(|| format!("surface window '{label}' not found"))?;
    window.set_focus().map_err(|e| format!("failed to focus surface window: {e}"))
}

/// List the labels of currently-tracked surface windows.
#[tauri::command]
pub fn list_surface_windows(
    manager: State<'_, SurfaceWindowManager>,
) -> Result<Vec<String>, String> {
    manager.labels()
}

/// Build the window label for a surface instance: `a2u-<surface>-<id>`.
pub fn surface_window_label(surface: &str, id: &str) -> String {
    format!("{SURFACE_WINDOW_PREFIX}{surface}-{id}")
}

/// Parse a window label back into `(surface, id)`. Returns `None` for the main
/// window or any non-surface label.
#[allow(dead_code)]
pub fn surface_from_window_label(label: &str) -> Option<(String, String)> {
    let rest = label.strip_prefix(SURFACE_WINDOW_PREFIX)?;
    let (surface, id) = rest.split_once('-')?;
    if surface.is_empty() || id.is_empty() {
        return None;
    }
    Some((surface.to_string(), id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{SurfaceWindowManager, surface_from_window_label, surface_window_label};

    #[test]
    fn surface_window_label_roundtrips() {
        let label = surface_window_label("workspace", "task-7");
        assert_eq!(label, "a2u-workspace-task-7");
        assert_eq!(
            surface_from_window_label(&label),
            Some(("workspace".to_string(), "task-7".to_string()))
        );
        // Main window + non-surface labels are not surface windows.
        assert_eq!(surface_from_window_label("main"), None);
        assert_eq!(surface_from_window_label("plugin-video"), None);
    }

    #[test]
    fn surface_window_label_rejects_empty_segments() {
        assert_eq!(surface_from_window_label("a2u--id"), None);
        assert_eq!(surface_from_window_label("a2u-surface-"), None);
        assert_eq!(surface_from_window_label("a2u-onlyone"), None);
    }

    #[test]
    fn manager_tracks_forgets_and_lists_labels() {
        let manager = SurfaceWindowManager::default();
        manager.track("a2u-workspace-1", "workspace").unwrap();
        manager.track("a2u-image-2", "image").unwrap();

        let mut labels = manager.labels().unwrap();
        labels.sort();
        assert_eq!(labels, vec!["a2u-image-2".to_string(), "a2u-workspace-1".to_string()]);

        let removed = manager.forget("a2u-workspace-1").unwrap();
        assert_eq!(removed.as_deref(), Some("workspace"));
        assert_eq!(manager.labels().unwrap().len(), 1);
    }
}
