use media_shared::*;
use reqwest::Client;

#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn list_files(&self) -> Result<Vec<MediaFile>, String> {
        let resp: ApiResponse<Vec<MediaFile>> = self
            .client
            .get(format!("{}/api/files", self.base_url))
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Parse error: {e}"))?;

        resp.data.ok_or_else(|| {
            resp.error.unwrap_or_else(|| "Unknown error".into())
        })
    }

    pub async fn scan_directory(&self, dir: &str) -> Result<Vec<MediaFile>, String> {
        let req = ScanRequest {
            directory: dir.to_string(),
        };
        let resp: ApiResponse<Vec<MediaFile>> = self
            .client
            .post(format!("{}/api/scan", self.base_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Parse error: {e}"))?;

        resp.data.ok_or_else(|| {
            resp.error.unwrap_or_else(|| "Unknown error".into())
        })
    }

    pub async fn start_encode(&self, req: &EncodeRequest) -> Result<EncodeJob, String> {
        let resp: ApiResponse<EncodeJob> = self
            .client
            .post(format!("{}/api/encode", self.base_url))
            .json(req)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Parse error: {e}"))?;

        resp.data.ok_or_else(|| {
            resp.error.unwrap_or_else(|| "Unknown error".into())
        })
    }

    pub async fn list_jobs(&self) -> Result<Vec<EncodeJob>, String> {
        let resp: ApiResponse<Vec<EncodeJob>> = self
            .client
            .get(format!("{}/api/jobs", self.base_url))
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Parse error: {e}"))?;

        resp.data.ok_or_else(|| {
            resp.error.unwrap_or_else(|| "Unknown error".into())
        })
    }
}
