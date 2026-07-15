use std::path::Path;

use anyhow::{Context, Result};
use reqwest::blocking::Client;

use super::types::{Page, Platform, SimpleRom};

pub struct RommClient {
    client: Client,
    base_url: String,
    token: String,
}

impl RommClient {
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .context("building HTTP client")?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub fn list_platforms(&self) -> Result<Vec<Platform>> {
        let resp = self
            .client
            .get(self.url("/api/platforms"))
            .header("Authorization", self.auth_header())
            .send()
            .context("sending platforms request")?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "platforms request failed: {} {}",
                resp.status(),
                resp.text().unwrap_or_default()
            );
        }

        let platforms: Vec<Platform> = resp.json().context("parsing platforms response")?;
        Ok(platforms)
    }

    pub fn list_roms(
        &self,
        platform_id: u64,
        offset: u64,
        limit: u64,
    ) -> Result<Page<SimpleRom>> {
        let resp = self
            .client
            .get(self.url("/api/roms"))
            .header("Authorization", self.auth_header())
            .query(&[
                ("platform_ids", platform_id.to_string()),
                ("with_files", "true".to_string()),
                ("limit", limit.to_string()),
                ("offset", offset.to_string()),
            ])
            .send()
            .context("sending roms request")?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "roms request failed: {} {}",
                resp.status(),
                resp.text().unwrap_or_default()
            );
        }

        let page: Page<SimpleRom> = resp.json().context("parsing roms response")?;
        Ok(page)
    }

    pub fn list_all_roms(&self, platform_id: u64) -> Result<Vec<SimpleRom>> {
        let mut all = Vec::new();
        let limit = 10000u64;
        let mut offset = 0u64;

        loop {
            let page = self.list_roms(platform_id, offset, limit)?;
            let count = page.items.len() as u64;
            all.extend(page.items);
            offset += count;
            if offset >= page.total || count == 0 {
                break;
            }
        }

        Ok(all)
    }

    #[allow(dead_code)]
    pub fn download_rom_content(
        &self,
        rom_id: u64,
        file_name: &str,
        dest: &Path,
    ) -> Result<u64> {

        let url = self.url(&format!(
            "/api/roms/{rom_id}/content/{}",
            urlencoding::encode(file_name)
        ));

        let mut resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .context("sending download request")?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "download failed: {} {}",
                resp.status(),
                resp.text().unwrap_or_default()
            );
        }

        let mut file = std::fs::File::create(dest)
            .with_context(|| format!("creating cache file {}", dest.display()))?;
        std::io::copy(&mut resp, &mut file).context("downloading ROM content")?;
        let total = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(total)
    }

    pub fn download_range(
        &self,
        rom_id: u64,
        file_name: &str,
        start: u64,
        end: u64,
    ) -> Result<Vec<u8>> {
        let url = self.url(&format!(
            "/api/roms/{rom_id}/content/{}",
            urlencoding::encode(file_name)
        ));

        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Range", format!("bytes={start}-{end}"))
            .send()
            .context("sending range request")?;

        let status = resp.status();
        if status == reqwest::StatusCode::PARTIAL_CONTENT {
            let bytes = resp.bytes().context("reading range response")?;
            Ok(bytes.to_vec())
        } else if status.is_success() {
            let bytes = resp.bytes().context("reading full response as fallback")?;
            Ok(bytes.to_vec())
        } else {
            anyhow::bail!(
                "range request failed: {status} {}",
                resp.text().unwrap_or_default()
            );
        }
    }
}
