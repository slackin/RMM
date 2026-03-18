use crate::AppState;
use axum::{
    extract::{Path, State},
    Json,
};
use media_shared::*;
use std::sync::Arc;

pub async fn health() -> Json<ApiResponse<String>> {
    Json(ApiResponse::ok("ok".into()))
}

pub async fn browse_directory(
    Json(req): Json<BrowseRequest>,
) -> Json<ApiResponse<Vec<DirEntry>>> {
    let dir_path = std::path::Path::new(&req.path);
    if !dir_path.is_dir() {
        return Json(ApiResponse::err("Path is not a valid directory"));
    }

    // Canonicalize to resolve symlinks and prevent traversal
    let canonical = match dir_path.canonicalize() {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::err(format!("Cannot resolve path: {e}"))),
    };

    let mut entries = Vec::new();
    let read_dir = match std::fs::read_dir(&canonical) {
        Ok(rd) => rd,
        Err(e) => return Json(ApiResponse::err(format!("Cannot read directory: {e}"))),
    };

    for entry in read_dir.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden entries
        if name.starts_with('.') {
            continue;
        }
        entries.push(DirEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir: meta.is_dir(),
        });
    }

    entries.sort_by(|a, b| {
        // Directories first, then alphabetical
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Json(ApiResponse::ok(entries))
}

pub async fn scan_directory(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ScanRequest>,
) -> Json<ApiResponse<Vec<MediaFile>>> {
    // Validate that the path is an existing directory
    let dir_path = std::path::Path::new(&req.directory);
    if !dir_path.is_dir() {
        return Json(ApiResponse::err("Path is not a valid directory"));
    }

    let paths = match crate::ffmpeg::scan_directory(&req.directory).await {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::err(e)),
    };

    let mut files = Vec::new();
    for path in paths {
        match crate::ffmpeg::probe_file(&path).await {
            Ok(media) => {
                if let Err(e) = state.db.upsert_file(&media) {
                    tracing::warn!("DB insert error for {path}: {e}");
                }
                files.push(media);
            }
            Err(e) => {
                tracing::warn!("Probe failed for {path}: {e}");
            }
        }
    }

    Json(ApiResponse::ok(files))
}

pub async fn list_files(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<MediaFile>>> {
    match state.db.list_files() {
        Ok(files) => Json(ApiResponse::ok(files)),
        Err(e) => Json(ApiResponse::err(format!("DB error: {e}"))),
    }
}

pub async fn get_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse<MediaFile>> {
    match state.db.get_file(&id) {
        Ok(Some(f)) => Json(ApiResponse::ok(f)),
        Ok(None) => Json(ApiResponse::err("File not found")),
        Err(e) => Json(ApiResponse::err(format!("DB error: {e}"))),
    }
}

pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse<bool>> {
    match state.db.delete_file(&id) {
        Ok(true) => Json(ApiResponse::ok(true)),
        Ok(false) => Json(ApiResponse::err("File not found")),
        Err(e) => Json(ApiResponse::err(format!("DB error: {e}"))),
    }
}

pub async fn start_encode(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EncodeRequest>,
) -> Json<ApiResponse<EncodeJob>> {
    // Look up source file
    let file = match state.db.get_file(&req.file_id) {
        Ok(Some(f)) => f,
        Ok(None) => return Json(ApiResponse::err("Source file not found")),
        Err(e) => return Json(ApiResponse::err(format!("DB error: {e}"))),
    };

    let job_id = new_id();
    let now = chrono::Utc::now().to_rfc3339();

    // Build output path: same dir, with codec/resolution suffix
    let input_path = std::path::PathBuf::from(&file.path);
    let stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".into());
    let ext = input_path
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "mkv".into());
    let parent = input_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".into());

    let vcodec_tag = serde_json::to_value(&req.video_codec)
        .unwrap()
        .as_str()
        .unwrap_or("video")
        .to_string();
    let res_tag = serde_json::to_value(&req.resolution)
        .unwrap()
        .as_str()
        .unwrap_or("orig")
        .to_string();
    let output_filename = format!("{stem}_{vcodec_tag}_{res_tag}.{ext}");
    let output_path = format!("{parent}/{output_filename}");

    let job = EncodeJob {
        id: job_id.clone(),
        file_id: req.file_id.clone(),
        status: JobStatus::Queued,
        progress_percent: 0.0,
        video_codec: req.video_codec,
        audio_codec: req.audio_codec,
        resolution: req.resolution,
        quality_crf: req.quality_crf,
        output_path: Some(output_path.clone()),
        error: None,
        created_at: now,
    };

    if let Err(e) = state.db.create_job(&job) {
        return Json(ApiResponse::err(format!("DB error: {e}")));
    }

    // Spawn encode in background
    let state2 = Arc::clone(&state);
    let input = file.path.clone();
    let duration = file.duration_secs;
    let vcodec = req.video_codec;
    let acodec = req.audio_codec;
    let resolution = req.resolution;
    let crf = req.quality_crf;
    let jid = job_id.clone();

    tokio::spawn(async move {
        // Acquire lock so we process one job at a time
        let _lock = state2.job_processor.lock().await;

        let _ = state2.db.update_job_status(&jid, JobStatus::Running, 0.0, None, None);

        let jid2 = jid.clone();
        let state3 = Arc::clone(&state2);

        let result = crate::ffmpeg::encode(
            &input,
            &output_path,
            vcodec,
            acodec,
            resolution,
            crf,
            duration,
            Arc::new(move |pct| {
                let _ = state3.db.update_job_status(
                    &jid2,
                    JobStatus::Running,
                    pct,
                    None,
                    None,
                );
            }),
        )
        .await;

        match result {
            Ok(()) => {
                // Re-read the job to get output_path
                let _ = state2.db.update_job_status(
                    &jid,
                    JobStatus::Completed,
                    100.0,
                    Some(&output_path),
                    None,
                );
                tracing::info!("Job {jid} completed");
            }
            Err(e) => {
                let _ = state2.db.update_job_status(
                    &jid,
                    JobStatus::Failed,
                    0.0,
                    None,
                    Some(&e),
                );
                tracing::error!("Job {jid} failed: {e}");
            }
        }
    });

    Json(ApiResponse::ok(job))
}

pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<EncodeJob>>> {
    match state.db.list_jobs() {
        Ok(jobs) => Json(ApiResponse::ok(jobs)),
        Err(e) => Json(ApiResponse::err(format!("DB error: {e}"))),
    }
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse<EncodeJob>> {
    match state.db.get_job(&id) {
        Ok(Some(j)) => Json(ApiResponse::ok(j)),
        Ok(None) => Json(ApiResponse::err("Job not found")),
        Err(e) => Json(ApiResponse::err(format!("DB error: {e}"))),
    }
}
