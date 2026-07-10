use super::*;

#[tool_router(router = ml_router)]
impl CullMcp {
    #[tool(
        description = "Find visually similar images using CLIP embeddings. Requires embeddings to be generated first."
    )]
    fn find_similar(&self, Parameters(params): Parameters<FindSimilarParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        let top_k = clamp_limit(params.limit.unwrap_or(10)) as usize;
        let model_id = params.model.as_deref().unwrap_or("clip-vit-b32");
        if crate::db_core::embeddings::embedding_model_spec(model_id).is_none() {
            return format!("Error: Unsupported embedding model '{}'", model_id);
        }

        let query = match state.db.get_embedding_vector(&params.image_id, model_id) {
            Ok(Some(vector)) => vector,
            Err(e) => return format!("Error: {}", e),
            Ok(None) => {
                return format!(
                    "Error: Image '{}' has no '{}' embedding. Run generate_embeddings first.",
                    params.image_id, model_id
                )
            }
        };
        match state.db.find_similar(&query, model_id, top_k * 2) {
            Ok(results) => {
                let r: Vec<serde_json::Value> = results
                .iter()
                .filter(|(id, _)| self.check_image_id_scope(id).unwrap_or(false))
                .take(top_k)
                .map(|(id, score)| serde_json::json!({"image_id": id, "similarity": score, "model": model_id}))
                .collect();
                serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get blur, focus, and exposure metrics for an image")]
    fn get_image_quality(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.get_image_quality_metrics(&params.image_id) {
            Ok(metrics) => serde_json::to_string(&metrics).unwrap_or_else(|_| "null".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get count of images with stored blur, focus, and exposure metrics")]
    fn get_quality_count(&self) -> String {
        let state = self.app_handle.state::<AppState>();
        let scoped_count = match self.scoped_images(&state) {
            Ok(Some(images)) => {
                let mut count = 0u32;
                for image in images {
                    match state.db.get_image_quality_metrics(&image.image.id) {
                        Ok(Some(_)) => count += 1,
                        Ok(None) => {}
                        Err(e) => return format!("Error: {}", e),
                    }
                }
                Some(count)
            }
            Ok(None) => None,
            Err(e) => return format!("Error: {}", e),
        };
        match state.db.quality_metrics_count() {
            Ok(count) => quality_count_for_mcp(count, scoped_count).to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "Get AI generation metadata (prompt, model, seed, provider) for an image")]
    fn get_generation_run(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        match state.db.get_generation_run_for_image(&params.image_id) {
            Ok(Some(run)) => generation_run_for_mcp(&run, &self.auth).to_string(),
            Ok(None) => "null".to_string(),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(
        description = "Manually attach AI generation metadata to an image (creates a generation run record)"
    )]
    fn set_generation_metadata(
        &self,
        Parameters(params): Parameters<SetGenerationMetadataParams>,
    ) -> String {
        match self.check_image_id_scope(&params.image_id) {
            Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
            Err(e) => return format!("Error: {}", e),
            _ => {}
        }
        let state = self.app_handle.state::<AppState>();
        let run = crate::db_core::models::GenerationRun {
            id: uuid::Uuid::new_v4().to_string(),
            prompt: Some(params.prompt),
            negative_prompt: None,
            provider: params.provider,
            model: params.model,
            settings_json: params.settings_json.unwrap_or_else(|| "{}".to_string()),
            seed: params.seed,
            parent_run_id: None,
            source_type: "manual".to_string(),
            source_path: None,
            raw_metadata_json: None,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            imported_at: chrono::Utc::now().to_rfc3339(),
        };
        let run_id = run.id.clone();
        if let Err(e) = state.db.insert_generation_run(&run) {
            return format!("Error creating run: {}", e);
        }
        if let Err(e) = state.db.link_image_to_run(&params.image_id, &run_id) {
            return format!("Error linking image: {}", e);
        }
        format!(
            "Created generation run {} for image {}",
            run_id, params.image_id
        )
    }

    #[tool(
        description = "Rescan all images for sidecar JSON files and backfill generation metadata. Returns the number of images linked."
    )]
    fn rescan_sidecars(&self) -> String {
        let state = self.app_handle.state::<AppState>();
        let images = match state.db.get_images_without_generation_run() {
            Ok(v) => v,
            Err(e) => return format!("Error: {}", e),
        };
        let mut linked = 0u32;
        for (image_id, file_path) in &images {
            let path = std::path::Path::new(file_path);
            if let Some(sidecar_path) = crate::db_core::sidecar::find_sidecar(path) {
                if crate::db_core::sidecar::link_sidecar_to_image(
                    &state.db,
                    image_id,
                    path,
                    &sidecar_path,
                    "sidecar",
                )
                .is_ok()
                {
                    linked += 1;
                }
            }
        }
        format!(
            "Rescanned {} images, linked {} sidecars",
            images.len(),
            linked
        )
    }

    #[tool(
        description = "Import all images from a folder into the library. Returns imported/skipped/error counts."
    )]
    fn import_folder(
        &self,
        Parameters(params): Parameters<crate::services::import::ImportFolderParams>,
    ) -> String {
        let scope = self.token_scope();
        if !tokens::folder_in_scope(&scope, &params.folder_path) {
            return "Error: Access denied — folder outside token scope".to_string();
        }
        let state = self.app_handle.state::<AppState>();
        match crate::services::import::import_folder(&state.db, &state.app_data_dir, params) {
            Ok(result) => {
                let value = serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}));
                json_response_for_mcp(value, &self.auth).to_string()
            }
            Err(e) => error_for_mcp(&e, &self.auth),
        }
    }

    #[tool(
        description = "Import specific image files into the library. Returns imported/skipped/error counts."
    )]
    fn import_files(
        &self,
        Parameters(params): Parameters<crate::services::import::ImportFilesParams>,
    ) -> String {
        let scope = self.token_scope();
        if params
            .file_paths
            .iter()
            .any(|path| !tokens::image_in_scope(&scope, path, &[]))
        {
            return "Error: Access denied — file outside token scope".to_string();
        }
        let state = self.app_handle.state::<AppState>();
        match crate::services::import::import_files(&state.db, &state.app_data_dir, params) {
            Ok(result) => {
                let value = serde_json::to_value(&result).unwrap_or_else(|_| serde_json::json!({}));
                json_response_for_mcp(value, &self.auth).to_string()
            }
            Err(e) => error_for_mcp(&e, &self.auth),
        }
    }

    #[tool(
        description = "Rescan all imported source folders for new/changed/missing files. Returns count of updated metadata."
    )]
    fn rescan_sources(&self, Parameters(_): Parameters<EmptyParams>) -> String {
        let state = self.app_handle.state::<AppState>();
        let app = self.app_handle.clone();

        let images = match state.db.list_images(100000, 0) {
            Ok(imgs) => imgs,
            Err(e) => return format!("Error: {}", e),
        };

        let total = images.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("rescan", total);
        let job_id_ret = job_id.clone();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let mut updated = 0u32;

            for (i, img) in images.iter().enumerate() {
                if cancel_token.is_cancelled() {
                    state.jobs.mark_cancelled(&job_id);
                    state.jobs.persist_terminal(&job_id, &state.db);
                    let _ = app.emit(
                        "job-status-changed",
                        serde_json::json!({"job_id": &job_id, "status": "cancelled"}),
                    );
                    return;
                }

                let path = std::path::Path::new(&img.path);
                if !path.exists() {
                    state.jobs.update_progress(&job_id, (i + 1) as u32, None);
                    continue;
                }

                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                let png_chunks = if ext == "png" {
                    crate::db_core::source_detection::read_png_text_chunks(path).unwrap_or_default()
                } else {
                    vec![]
                };

                let detection =
                    crate::db_core::source_detection::detect_source(filename, &png_chunks, path);
                if detection.source_label.is_some() {
                    let aspect_ratio = img.image.width as f64 / img.image.height.max(1) as f64;
                    let orientation = if (aspect_ratio - 1.0).abs() < 0.05 {
                        "square"
                    } else if aspect_ratio > 1.0 {
                        "landscape"
                    } else {
                        "portrait"
                    };
                    let megapixels =
                        (img.image.width as f64 * img.image.height as f64) / 1_000_000.0;

                    let _ = state.db.update_source_detection(
                        &img.image.id,
                        detection.source_label.as_deref(),
                        detection.confidence,
                        &detection.to_evidence_json(),
                        detection.is_ai_generated,
                        detection.ai_prompt.as_deref(),
                        aspect_ratio,
                        orientation,
                        megapixels,
                    );
                    updated += 1;
                }
                state
                    .jobs
                    .update_progress(&job_id, (i + 1) as u32, Some(filename));
            }

            state.jobs.complete(&job_id);
            state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit(
                "job-status-changed",
                serde_json::json!({"job_id": &job_id, "status": "completed", "updated": updated}),
            );
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total}).to_string()
    }

    #[tool(
        description = "Download a local embedding model. Supports 'clip-vit-b32' and 'dinov2-vits14'. Returns a background job id."
    )]
    fn download_embedding_model(
        &self,
        Parameters(params): Parameters<DownloadEmbeddingModelParams>,
    ) -> String {
        let model_id = params
            .model
            .unwrap_or_else(|| crate::db_core::embeddings::CLIP_MODEL_ID.to_string());
        let Some(spec) = crate::db_core::embeddings::embedding_model_spec(&model_id) else {
            return format!("Error: Unsupported embedding model '{}'", model_id);
        };
        let state = self.app_handle.state::<AppState>();
        let model_path = {
            let engine = state.embedding_engine.lock();
            match engine.model_path_for(spec.model_id) {
                Ok(path) => path,
                Err(e) => return format!("Error: {}", e),
            }
        };

        if model_path.exists() {
            return model_download_response_for_mcp(
                "already_downloaded",
                None,
                &spec,
                &model_path,
                &self.auth,
            )
            .to_string();
        }

        let (job_id, _cancel_token) = state
            .jobs
            .create_job(&format!("{}-download", spec.model_id), 0);
        let Some(control) = state.jobs.control_for(&job_id) else {
            return format!("Error: Download job '{}' not found", job_id);
        };
        let app = self.app_handle.clone();
        let job_id_ret = job_id.clone();
        let model_path_for_task = model_path.clone();
        let url = spec.url.to_string();
        let model_id_for_task = spec.model_id.to_string();
        let display_name = spec.display_name.to_string();
        let verification = spec.download_verification();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let client = reqwest::Client::new();
            let result = crate::services::model_download::download_model_file_verified_controlled(
                &client,
                &url,
                &model_path_for_task,
                Some(&verification),
                &control,
                |progress| {
                    state.jobs.update_progress(
                        &job_id,
                        progress.downloaded.min(u32::MAX as u64) as u32,
                        Some(&format!("Downloading {}", display_name)),
                    );
                    let _ = app.emit(
                        "model-download-progress",
                        serde_json::json!({
                            "job_id": &job_id,
                            "model": &model_id_for_task,
                            "downloaded": progress.downloaded,
                            "total": progress.total,
                            "status": progress.status,
                            "resumable": progress.resumable,
                        }),
                    );
                },
            )
            .await;

            match result {
                Ok(_) => {
                    let load_result = {
                        let mut engine = state.embedding_engine.lock();
                        engine.load_model_for(&model_id_for_task)
                    };
                    match load_result {
                        Ok(()) => state.jobs.complete(&job_id),
                        Err(e) => state.jobs.fail(&job_id, &e),
                    }
                }
                Err(e) => {
                    if control.cancellation_token().is_cancelled() {
                        state.jobs.mark_cancelled(&job_id);
                    } else {
                        state.jobs.fail(&job_id, &e);
                    }
                }
            }
            state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit(
            "job-status-changed",
            serde_json::json!({"job_id": &job_id, "status": state.jobs.get(&job_id).map(|j| j.status)}),
        );
        });

        model_download_response_for_mcp("started", Some(job_id_ret), &spec, &model_path, &self.auth)
            .to_string()
    }

    #[tool(
        description = "Generate visual embeddings for images (required for find_similar). Supports CLIP and DINOv2. Returns a background job id."
    )]
    fn generate_embeddings(
        &self,
        Parameters(params): Parameters<GenerateEmbeddingsParams>,
    ) -> String {
        for image_id in &params.image_ids {
            match self.check_image_id_scope(image_id) {
                Ok(false) => {
                    return format!(
                        "Error: Access denied — image '{}' outside token scope",
                        image_id
                    )
                }
                Err(e) => return format!("Error: {}", e),
                _ => {}
            }
        }
        let state = self.app_handle.state::<AppState>();
        let model_id = params
            .model
            .unwrap_or_else(|| crate::db_core::embeddings::CLIP_MODEL_ID.to_string());
        if crate::db_core::embeddings::embedding_model_spec(&model_id).is_none() {
            return format!("Error: Unsupported embedding model '{}'", model_id);
        }

        {
            let mut engine = state.embedding_engine.lock();
            if let Err(e) = engine.load_model_for(&model_id) {
                return format!("Error loading model: {}", e);
            }
        }

        let total = params.image_ids.len() as u32;
        let (job_id, cancel_token) = state.jobs.create_job("embeddings", total);
        let job_id_ret = job_id.clone();
        let app = self.app_handle.clone();
        let image_ids = params.image_ids.clone();
        let model_id_for_task = model_id.clone();

        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let result = crate::services::model_pipeline::run_embedding_model(
                crate::services::model_pipeline::EmbeddingRunRequest {
                    db: &state.db,
                    app_data_dir: &state.app_data_dir,
                    embedding_engine: &state.embedding_engine,
                    jobs: Some(&state.jobs),
                    job_id: Some(&job_id),
                    cancel: Some(&cancel_token),
                    app: Some(&app),
                    model_id: &model_id_for_task,
                    image_ids: &image_ids,
                },
            );
            match result {
                Ok(result) if result.status == "cancelled" => state.jobs.mark_cancelled(&job_id),
                Ok(_) => state.jobs.complete(&job_id),
                Err(e) => state.jobs.fail(&job_id, &e),
            }
            state.jobs.persist_terminal(&job_id, &state.db);
            let _ = app.emit(
            "job-status-changed",
            serde_json::json!({"job_id": &job_id, "status": state.jobs.get(&job_id).map(|j| j.status)}),
        );
        });

        serde_json::json!({"job_id": job_id_ret, "total_images": total, "model": model_id})
            .to_string()
    }
}

pub(super) fn router() -> super::ToolRouter<super::CullMcp> {
    super::CullMcp::ml_router()
}
