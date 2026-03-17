# Media Manager

A client/server tool for managing, analyzing, and re-encoding a remote media library using FFmpeg.

## Architecture

```
┌──────────────────┐         HTTP/JSON         ┌──────────────────┐
│   GUI Client     │ ◄──────────────────────► │     Server       │
│  (egui/eframe)   │       REST API :9090      │  (Axum + FFmpeg) │
│  Win / Linux     │                           │  Linux           │
└──────────────────┘                           └──────────────────┘
```

- **shared/** — Common types (media files, codecs, jobs, API models)
- **server/** — Axum HTTP server with SQLite persistence and FFmpeg integration
- **client/** — Cross-platform native GUI (egui/eframe)

## Features

- **Library scanning** — Recursively scan directories for media files (video + audio)
- **Media probing** — Uses `ffprobe` to extract codec, resolution, duration, bitrate info
- **Re-encoding** — Submit FFmpeg encode jobs with configurable:
  - **Video codecs**: H.264, H.265/HEVC, AV1, VP9, or copy (no re-encode)
  - **Audio codecs**: AAC, Opus, FLAC, MP3, or copy (no re-encode)
  - **Resolution profiles**: 4K (3840×2160), 2K (2560×1440), 1080p, 720p, or original
  - **Quality (CRF)**: 0–51 slider (lower = higher quality)
- **Job tracking** — Real-time progress monitoring for active encode jobs
- **Codec switching** — Change codecs without altering resolution (select "Original" + desired codec)

## Prerequisites

- **Rust** (stable toolchain)
- **FFmpeg** & **ffprobe** installed on the server machine
- Linux desktop libraries for the GUI client (on Linux): `libgtk-3-dev` or equivalent

## Build

```bash
# Build everything
cargo build --release

# Server binary: target/release/media-server
# Client binary: target/release/media-client
```

## Run

### Server (Linux)

```bash
# Ensure ffmpeg is available
ffmpeg -version

# Start the server (listens on 0.0.0.0:9090)
./target/release/media-server
```

The server creates `media_manager.db` (SQLite) in the current directory.

### Client (Windows or Linux)

```bash
./target/release/media-client
```

1. Enter the server URL (e.g., `http://192.168.1.100:9090`)
2. Click **Connect & Refresh**
3. Go to **Library** tab → enter a directory path on the server → click **Scan**
4. Select a file → switch to **Encode** tab → configure codec/resolution/quality
5. Click **Start Encoding** → monitor progress in the **Jobs** tab

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | Health check |
| POST | `/api/scan` | Scan directory for media files |
| GET | `/api/files` | List all known media files |
| GET | `/api/files/{id}` | Get single file details |
| DELETE | `/api/files/{id}` | Remove file from library |
| POST | `/api/encode` | Submit an encoding job |
| GET | `/api/jobs` | List all encoding jobs |
| GET | `/api/jobs/{id}` | Get single job status |
