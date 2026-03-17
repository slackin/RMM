use media_shared::{AudioCodec, EncodeJob, JobStatus, MediaFile, ResolutionProfile, VideoCodec};
use rusqlite::{Connection, params};
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn migrate(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS media_files (
                id          TEXT PRIMARY KEY,
                path        TEXT NOT NULL UNIQUE,
                filename    TEXT NOT NULL,
                size_bytes  INTEGER NOT NULL DEFAULT 0,
                duration_secs REAL,
                video_codec TEXT,
                audio_codec TEXT,
                width       INTEGER,
                height      INTEGER,
                bitrate     INTEGER,
                format      TEXT
            );

            CREATE TABLE IF NOT EXISTS encode_jobs (
                id              TEXT PRIMARY KEY,
                file_id         TEXT NOT NULL REFERENCES media_files(id),
                status          TEXT NOT NULL DEFAULT 'queued',
                progress_percent REAL NOT NULL DEFAULT 0,
                video_codec     TEXT NOT NULL,
                audio_codec     TEXT NOT NULL,
                resolution      TEXT NOT NULL,
                quality_crf     INTEGER,
                output_path     TEXT,
                error           TEXT,
                created_at      TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    // ── Media files ─────────────────────────────────────────────────────

    pub fn upsert_file(&self, f: &MediaFile) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO media_files (id, path, filename, size_bytes, duration_secs,
             video_codec, audio_codec, width, height, bitrate, format)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(path) DO UPDATE SET
               size_bytes=excluded.size_bytes,
               duration_secs=excluded.duration_secs,
               video_codec=excluded.video_codec,
               audio_codec=excluded.audio_codec,
               width=excluded.width,
               height=excluded.height,
               bitrate=excluded.bitrate,
               format=excluded.format",
            params![
                f.id,
                f.path,
                f.filename,
                f.size_bytes,
                f.duration_secs,
                f.video_codec,
                f.audio_codec,
                f.width,
                f.height,
                f.bitrate,
                f.format
            ],
        )?;
        Ok(())
    }

    pub fn list_files(&self) -> Result<Vec<MediaFile>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, path, filename, size_bytes, duration_secs,
                    video_codec, audio_codec, width, height, bitrate, format
             FROM media_files ORDER BY filename",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MediaFile {
                id: row.get(0)?,
                path: row.get(1)?,
                filename: row.get(2)?,
                size_bytes: row.get(3)?,
                duration_secs: row.get(4)?,
                video_codec: row.get(5)?,
                audio_codec: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                bitrate: row.get(9)?,
                format: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_file(&self, id: &str) -> Result<Option<MediaFile>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, path, filename, size_bytes, duration_secs,
                    video_codec, audio_codec, width, height, bitrate, format
             FROM media_files WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(MediaFile {
                id: row.get(0)?,
                path: row.get(1)?,
                filename: row.get(2)?,
                size_bytes: row.get(3)?,
                duration_secs: row.get(4)?,
                video_codec: row.get(5)?,
                audio_codec: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                bitrate: row.get(9)?,
                format: row.get(10)?,
            })
        })?;
        match rows.next() {
            Some(Ok(f)) => Ok(Some(f)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    pub fn delete_file(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM media_files WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    // ── Encode jobs ─────────────────────────────────────────────────────

    pub fn create_job(&self, job: &EncodeJob) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO encode_jobs (id, file_id, status, progress_percent,
             video_codec, audio_codec, resolution, quality_crf, output_path, error, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                job.id,
                job.file_id,
                serde_json::to_value(&job.status).unwrap().as_str(),
                job.progress_percent,
                serde_json::to_value(&job.video_codec).unwrap().as_str(),
                serde_json::to_value(&job.audio_codec).unwrap().as_str(),
                serde_json::to_value(&job.resolution).unwrap().as_str(),
                job.quality_crf,
                job.output_path,
                job.error,
                job.created_at
            ],
        )?;
        Ok(())
    }

    pub fn update_job_status(
        &self,
        id: &str,
        status: JobStatus,
        progress: f32,
        output_path: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let status_str = serde_json::to_value(&status).unwrap();
        conn.execute(
            "UPDATE encode_jobs SET status=?2, progress_percent=?3,
             output_path=?4, error=?5 WHERE id=?1",
            params![id, status_str.as_str(), progress, output_path, error],
        )?;
        Ok(())
    }

    pub fn list_jobs(&self) -> Result<Vec<EncodeJob>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, file_id, status, progress_percent, video_codec, audio_codec,
                    resolution, quality_crf, output_path, error, created_at
             FROM encode_jobs ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| Self::row_to_job(row))?;
        rows.collect()
    }

    pub fn get_job(&self, id: &str) -> Result<Option<EncodeJob>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, file_id, status, progress_percent, video_codec, audio_codec,
                    resolution, quality_crf, output_path, error, created_at
             FROM encode_jobs WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| Self::row_to_job(row))?;
        match rows.next() {
            Some(Ok(j)) => Ok(Some(j)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    fn row_to_job(row: &rusqlite::Row) -> Result<EncodeJob, rusqlite::Error> {
        let status_str: String = row.get(2)?;
        let vcodec_str: String = row.get(4)?;
        let acodec_str: String = row.get(5)?;
        let res_str: String = row.get(6)?;

        Ok(EncodeJob {
            id: row.get(0)?,
            file_id: row.get(1)?,
            status: serde_json::from_value(serde_json::Value::String(status_str))
                .unwrap_or(JobStatus::Failed),
            progress_percent: row.get(3)?,
            video_codec: serde_json::from_value(serde_json::Value::String(vcodec_str))
                .unwrap_or(VideoCodec::H264),
            audio_codec: serde_json::from_value(serde_json::Value::String(acodec_str))
                .unwrap_or(AudioCodec::AAC),
            resolution: serde_json::from_value(serde_json::Value::String(res_str))
                .unwrap_or(ResolutionProfile::Original),
            quality_crf: row.get(7)?,
            output_path: row.get(8)?,
            error: row.get(9)?,
            created_at: row.get(10)?,
        })
    }
}
