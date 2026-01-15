//! Package repository management
//!
//! Status: PLACEHOLDER - Implementation pending Month 3

/// Package repository source
#[derive(Debug, Clone)]
pub struct Repository {
    pub name: String,
    pub url: String,
    pub enabled: bool,
}

/// Package metadata from repository
#[derive(Debug, Clone)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub size: u64,
    pub sha256: String,
    pub dependencies: Vec<String>,
}

// Implementation will include:
// - fetch_index() -> Result<Vec<PackageMetadata>>
// - download_package(name: &str) -> Result<PathBuf>
// - verify_checksum(path: &Path, expected: &str) -> Result<bool>
