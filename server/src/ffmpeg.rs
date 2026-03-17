use media_shared::{AudioCodec, MediaFile, ResolutionProfile, VideoCodec};
use std::path::Path;
use std::sync::Arc;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Probe a media file using ffprobe and return a populated MediaFile.
pub async fn probe_file(path: &str) -> Result<MediaFile, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run ffprobe: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("Invalid ffprobe JSON: {e}"))?;

    let filename = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let format = &json["format"];
    let size_bytes = format["size"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let duration_secs = format["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok());
    let bitrate = format["bit_rate"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok());
    let format_name = format["format_name"].as_str().map(String::from);

    let streams = json["streams"].as_array();

    let video_stream = streams
        .and_then(|s| s.iter().find(|s| s["codec_type"].as_str() == Some("video")));
    let audio_stream = streams
        .and_then(|s| s.iter().find(|s| s["codec_type"].as_str() == Some("audio")));

    let video_codec = video_stream
        .and_then(|s| s["codec_name"].as_str())
        .map(String::from);
    let width = video_stream.and_then(|s| s["width"].as_u64().map(|v| v as u32));
    let height = video_stream.and_then(|s| s["height"].as_u64().map(|v| v as u32));

    let audio_codec = audio_stream
        .and_then(|s| s["codec_name"].as_str())
        .map(String::from);

    Ok(MediaFile {
        id: media_shared::new_id(),
        path: path.to_string(),
        filename,
        size_bytes,
        duration_secs,
        video_codec,
        audio_codec,
        width,
        height,
        bitrate,
        format: format_name,
    })
}

/// Build and run an ffmpeg encode command. Calls `on_progress` with percentage.
pub async fn encode(
    input_path: &str,
    output_path: &str,
    video_codec: VideoCodec,
    audio_codec: AudioCodec,
    resolution: ResolutionProfile,
    quality_crf: Option<u8>,
    duration_secs: Option<f64>,
    on_progress: Arc<dyn Fn(f32) + Send + Sync>,
) -> Result<(), String> {
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        input_path.into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
    ];

    // Video codec
    args.extend(["-c:v".into(), video_codec.ffmpeg_encoder().into()]);

    // Audio codec
    args.extend(["-c:a".into(), audio_codec.ffmpeg_encoder().into()]);

    // Resolution scaling
    if let Some((w, h)) = resolution.dimensions() {
        args.extend([
            "-vf".into(),
            format!("scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2"),
        ]);
    }

    // CRF quality
    if video_codec != VideoCodec::Copy {
        let crf = quality_crf.unwrap_or(23);
        args.extend(["-crf".into(), crf.to_string()]);
    }

    args.push(output_path.into());

    tracing::info!("ffmpeg {}", args.join(" "));

    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start ffmpeg: {e}"))?;

    // Parse progress from stdout
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let total = duration_secs.unwrap_or(0.0);
        let progress_cb = Arc::clone(&on_progress);

        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                if line.starts_with("out_time_us=") {
                    if let Ok(us) = line.trim_start_matches("out_time_us=").parse::<f64>() {
                        let secs = us / 1_000_000.0;
                        if total > 0.0 {
                            let pct = (secs / total * 100.0).min(100.0) as f32;
                            progress_cb(pct);
                        }
                    }
                }
            }
        });
    }

    let result = child
        .wait()
        .await
        .map_err(|e| format!("ffmpeg process error: {e}"))?;

    if result.success() {
        on_progress(100.0);
        Ok(())
    } else {
        Err(format!("ffmpeg exited with code {:?}", result.code()))
    }
}

/// Scan a directory for media files (common video/audio extensions).
pub async fn scan_directory(dir: &str) -> Result<Vec<String>, String> {
    let dir = dir.to_string();
    tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        let extensions = [
            "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "mpg", "mpeg",
            "mp3", "flac", "wav", "aac", "ogg", "m4a", "wma", "opus",
        ];
        visit_dir(Path::new(&dir), &extensions, &mut files)?;
        files.sort();
        Ok(files)
    })
    .await
    .map_err(|e| format!("Scan task failed: {e}"))?
}

fn visit_dir(dir: &Path, extensions: &[&str], out: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Dir entry error: {e}"))?;
        let ft = entry
            .file_type()
            .map_err(|e| format!("File type error: {e}"))?;
        let path = entry.path();
        if ft.is_dir() {
            visit_dir(&path, extensions, out)?;
        } else if ft.is_file() || ft.is_symlink() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
                    if let Some(s) = path.to_str() {
                        out.push(s.to_string());
                    }
                }
            }
        }
    }
    Ok(())
}
