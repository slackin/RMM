use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Media file representation ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub id: String,
    pub path: String,
    pub filename: String,
    pub size_bytes: u64,
    pub duration_secs: Option<f64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bitrate: Option<u64>,
    pub format: Option<String>,
}

// ── Encoding profiles ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResolutionProfile {
    #[serde(rename = "4k")]
    UHD4K,
    #[serde(rename = "2k")]
    QHD2K,
    #[serde(rename = "1080p")]
    FHD1080,
    #[serde(rename = "720p")]
    HD720,
    /// Keep original resolution
    #[serde(rename = "original")]
    Original,
}

impl ResolutionProfile {
    /// Returns (width, height) for the profile, or None for Original.
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        match self {
            Self::UHD4K => Some((3840, 2160)),
            Self::QHD2K => Some((2560, 1440)),
            Self::FHD1080 => Some((1920, 1080)),
            Self::HD720 => Some((1280, 720)),
            Self::Original => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::UHD4K => "4K (3840×2160)",
            Self::QHD2K => "2K (2560×1440)",
            Self::FHD1080 => "1080p (1920×1080)",
            Self::HD720 => "720p (1280×720)",
            Self::Original => "Original",
        }
    }

    pub const ALL: &'static [ResolutionProfile] = &[
        Self::UHD4K,
        Self::QHD2K,
        Self::FHD1080,
        Self::HD720,
        Self::Original,
    ];
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VideoCodec {
    #[serde(rename = "h264")]
    H264,
    #[serde(rename = "h265")]
    H265,
    #[serde(rename = "av1")]
    AV1,
    #[serde(rename = "vp9")]
    VP9,
    /// Copy video stream without re-encoding
    #[serde(rename = "copy")]
    Copy,
}

impl VideoCodec {
    pub fn ffmpeg_encoder(&self) -> &'static str {
        match self {
            Self::H264 => "libx264",
            Self::H265 => "libx265",
            Self::AV1 => "libsvtav1",
            Self::VP9 => "libvpx-vp9",
            Self::Copy => "copy",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::H264 => "H.264 (AVC)",
            Self::H265 => "H.265 (HEVC)",
            Self::AV1 => "AV1",
            Self::VP9 => "VP9",
            Self::Copy => "Copy (no re-encode)",
        }
    }

    pub const ALL: &'static [VideoCodec] = &[
        Self::H264,
        Self::H265,
        Self::AV1,
        Self::VP9,
        Self::Copy,
    ];
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioCodec {
    #[serde(rename = "aac")]
    AAC,
    #[serde(rename = "opus")]
    Opus,
    #[serde(rename = "flac")]
    FLAC,
    #[serde(rename = "mp3")]
    MP3,
    /// Copy audio stream without re-encoding
    #[serde(rename = "copy")]
    Copy,
}

impl AudioCodec {
    pub fn ffmpeg_encoder(&self) -> &'static str {
        match self {
            Self::AAC => "aac",
            Self::Opus => "libopus",
            Self::FLAC => "flac",
            Self::MP3 => "libmp3lame",
            Self::Copy => "copy",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AAC => "AAC",
            Self::Opus => "Opus",
            Self::FLAC => "FLAC",
            Self::MP3 => "MP3",
            Self::Copy => "Copy (no re-encode)",
        }
    }

    pub const ALL: &'static [AudioCodec] = &[
        Self::AAC,
        Self::Opus,
        Self::FLAC,
        Self::MP3,
        Self::Copy,
    ];
}

// ── Encoding job ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodeRequest {
    pub file_id: String,
    pub video_codec: VideoCodec,
    pub audio_codec: AudioCodec,
    pub resolution: ResolutionProfile,
    /// CRF quality value (lower = better). Typical: 18-28 for h264/h265.
    pub quality_crf: Option<u8>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodeJob {
    pub id: String,
    pub file_id: String,
    pub status: JobStatus,
    pub progress_percent: f32,
    pub video_codec: VideoCodec,
    pub audio_codec: AudioCodec,
    pub resolution: ResolutionProfile,
    pub quality_crf: Option<u8>,
    pub output_path: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
}

// ── API requests / responses ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRequest {
    pub directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
