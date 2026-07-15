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
    pub name: Option<String>,
    pub slug: Option<String>,
    pub fs_name: String,
    pub fs_name_no_ext: String,
    pub fs_extension: String,
    pub fs_size_bytes: u64,
    pub platform_id: u64,
    pub platform_slug: String,
    pub platform_fs_slug: String,
    pub has_simple_single_file: bool,
    pub has_nested_single_file: bool,
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
    pub file_path: String,
    pub file_size_bytes: u64,
    pub is_top_level: bool,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}
