// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

pub mod agent_snapshots;
#[allow(dead_code)]
pub mod ai;
pub mod asset_events;
pub mod audit;
pub mod clipboard_monitor;
#[cfg(target_os = "macos")]
pub mod clipboard_monitor_macos;
#[allow(dead_code)]
pub mod curation;
pub mod display;
pub mod export;
pub mod generation;
pub mod import;
pub mod jobs;
#[allow(dead_code)]
pub mod library;
pub mod model_download;
pub mod model_pipeline;
pub mod ocr;
pub mod sessions;
pub mod tokens;
pub mod undo;

use crate::db_core::db::Database;
use crate::db_core::detection::DetectionEngine;
use crate::db_core::embeddings::EmbeddingEngine;
use crate::db_core::secrets::SecretStore;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pagination {
    pub offset: u32,
    pub limit: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 50,
        }
    }
}

impl Pagination {
    pub fn clamped(offset: u32, limit: u32) -> Self {
        Self {
            offset,
            limit: limit.min(250).max(1),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub offset: u32,
    pub has_more: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Engine error: {0}")]
    Engine(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<ServiceError> for String {
    fn from(e: ServiceError) -> String {
        e.to_string()
    }
}

#[allow(dead_code)]
pub struct ServiceContext<'a> {
    pub db: &'a Database,
    pub app_data_dir: &'a PathBuf,
    pub embedding_engine: &'a Mutex<EmbeddingEngine>,
    pub detection_engine: &'a Mutex<DetectionEngine>,
    pub safety_engine: &'a Mutex<DetectionEngine>,
    pub secrets: &'a dyn SecretStore,
    pub app_handle: Option<tauri::AppHandle>,
}

impl<'a> ServiceContext<'a> {
    pub fn from_app_state(
        state: &'a crate::AppState,
        app_handle: Option<tauri::AppHandle>,
    ) -> Self {
        Self {
            db: &state.db,
            app_data_dir: &state.app_data_dir,
            embedding_engine: &state.embedding_engine,
            detection_engine: &state.detection_engine,
            safety_engine: &state.safety_engine,
            secrets: state.secrets.as_ref(),
            app_handle,
        }
    }
}
