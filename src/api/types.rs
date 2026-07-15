use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Platform {
    pub id: u64,
    pub name: String,
    pub slug: String,
    pub fs_slug: String,
    pub rom_count: u64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SimpleRom {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    pub fs_name: String,
    #[serde(default)]
    pub fs_name_no_ext: String,
    #[serde(default)]
    pub fs_extension: String,
    pub fs_size_bytes: u64,
    pub platform_id: u64,
    #[serde(default)]
    pub platform_slug: String,
    #[serde(default)]
    pub platform_fs_slug: String,
    #[serde(default)]
    pub is_top_level: bool,
    #[serde(default)]
    pub has_simple_single_file: bool,
    #[serde(default)]
    pub has_nested_single_file: bool,
    #[serde(default)]
    pub has_multiple_files: bool,
    #[serde(default)]
    pub files: Vec<RomFile>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RomFile {
    pub id: u64,
    pub rom_id: u64,
    pub file_name: String,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub file_size_bytes: u64,
    #[serde(default)]
    pub is_top_level: bool,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    #[serde(default)]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
}
