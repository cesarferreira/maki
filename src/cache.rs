use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::target::Target;

/// Cache entry for a single Makefile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// SHA256 hash of the Makefile content
    pub content_hash: String,
    /// Last modification time of the Makefile
    pub modified_time: u64,
    /// Cached targets from this Makefile
    pub targets: Vec<Target>,
}

/// The complete cache structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cache {
    /// Version of the cache format (for future compatibility)
    pub version: u32,
    /// Map of absolute file paths to their cache entries
    pub entries: HashMap<String, CacheEntry>,
}

impl Cache {
    const CURRENT_VERSION: u32 = 1;
    const CACHE_FILENAME: &'static str = "maki_cache.json";

    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            entries: HashMap::new(),
        }
    }

    /// Get the cache directory path
    pub fn cache_dir() -> Option<PathBuf> {
        dirs::cache_dir().map(|p| p.join("maki"))
    }

    /// Get the full path to the cache file
    pub fn cache_file_path() -> Option<PathBuf> {
        Self::cache_dir().map(|p| p.join(Self::CACHE_FILENAME))
    }

    /// Load the cache from disk
    pub fn load() -> Result<Self> {
        let cache_path = Self::cache_file_path()
            .context("Could not determine cache directory")?;

        if !cache_path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&cache_path)
            .with_context(|| format!("Failed to read cache file: {}", cache_path.display()))?;

        let cache: Self = serde_json::from_str(&content)
            .with_context(|| "Failed to parse cache file")?;

        // Check version compatibility
        if cache.version != Self::CURRENT_VERSION {
            // Incompatible version, return fresh cache
            return Ok(Self::new());
        }

        Ok(cache)
    }

    /// Save the cache to disk
    pub fn save(&self) -> Result<()> {
        let cache_dir = Self::cache_dir()
            .context("Could not determine cache directory")?;

        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)
                .with_context(|| format!("Failed to create cache directory: {}", cache_dir.display()))?;
        }

        let cache_path = cache_dir.join(Self::CACHE_FILENAME);
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize cache")?;

        fs::write(&cache_path, content)
            .with_context(|| format!("Failed to write cache file: {}", cache_path.display()))?;

        Ok(())
    }

    /// Get cached targets for a Makefile if the cache is still valid
    pub fn get(&self, makefile_path: &Path) -> Option<&Vec<Target>> {
        let abs_path = makefile_path.canonicalize().ok()?;
        let path_str = abs_path.to_string_lossy().to_string();

        let entry = self.entries.get(&path_str)?;

        // Verify the cache is still valid
        if self.is_entry_valid(makefile_path, entry) {
            Some(&entry.targets)
        } else {
            None
        }
    }

    /// Check if a cache entry is still valid
    fn is_entry_valid(&self, makefile_path: &Path, entry: &CacheEntry) -> bool {
        // Check if file still exists and hash matches
        if let Ok(content) = fs::read_to_string(makefile_path) {
            let current_hash = compute_hash(&content);
            current_hash == entry.content_hash
        } else {
            false
        }
    }

    /// Store targets in the cache for a Makefile
    pub fn set(&mut self, makefile_path: &Path, targets: Vec<Target>) -> Result<()> {
        let abs_path = makefile_path.canonicalize()
            .with_context(|| format!("Failed to get absolute path for: {}", makefile_path.display()))?;

        let content = fs::read_to_string(makefile_path)
            .with_context(|| format!("Failed to read Makefile: {}", makefile_path.display()))?;

        let content_hash = compute_hash(&content);

        let modified_time = fs::metadata(makefile_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let entry = CacheEntry {
            content_hash,
            modified_time,
            targets,
        };

        self.entries.insert(abs_path.to_string_lossy().to_string(), entry);
        Ok(())
    }

    /// Remove a specific entry from the cache
    #[allow(dead_code)]
    pub fn invalidate(&mut self, makefile_path: &Path) {
        if let Ok(abs_path) = makefile_path.canonicalize() {
            self.entries.remove(&abs_path.to_string_lossy().to_string());
        }
    }

    /// Clear the entire cache
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Remove stale entries (files that no longer exist)
    #[allow(dead_code)]
    pub fn prune(&mut self) {
        self.entries.retain(|path, _| Path::new(path).exists());
    }

    /// Get cache statistics
    #[allow(dead_code)]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.entries.len(),
            total_targets: self.entries.values().map(|e| e.targets.len()).sum(),
        }
    }
}

/// Statistics about the cache
#[derive(Debug)]
#[allow(dead_code)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_targets: usize,
}

/// Compute SHA256 hash of content
pub fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Delete the cache file from disk
#[allow(dead_code)]
pub fn clear_cache() -> Result<()> {
    if let Some(cache_path) = Cache::cache_file_path() {
        if cache_path.exists() {
            fs::remove_file(&cache_path)
                .with_context(|| format!("Failed to delete cache file: {}", cache_path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_compute_hash() {
        let hash1 = compute_hash("hello world");
        let hash2 = compute_hash("hello world");
        let hash3 = compute_hash("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA256 produces 64 hex chars
    }

    #[test]
    fn test_cache_new() {
        let cache = Cache::new();
        assert_eq!(cache.version, Cache::CURRENT_VERSION);
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn test_cache_set_and_get() {
        let mut cache = Cache::new();

        // Create a temporary Makefile
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "build:\n\techo building").unwrap();

        let targets = vec![Target::new(
            "build".to_string(),
            Some("Build target".to_string()),
            temp_file.path().to_path_buf(),
            1,
        )];

        // Set cache entry
        cache.set(temp_file.path(), targets.clone()).unwrap();

        // Get cache entry
        let cached = cache.get(temp_file.path());
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
        assert_eq!(cached.unwrap()[0].name, "build");
    }

    #[test]
    fn test_cache_invalidation_on_content_change() {
        let mut cache = Cache::new();

        // Create a temporary Makefile
        let temp_dir = TempDir::new().unwrap();
        let makefile_path = temp_dir.path().join("Makefile");
        fs::write(&makefile_path, "build:\n\techo building").unwrap();

        let targets = vec![Target::new(
            "build".to_string(),
            None,
            makefile_path.clone(),
            1,
        )];

        cache.set(&makefile_path, targets).unwrap();

        // Verify cache hit
        assert!(cache.get(&makefile_path).is_some());

        // Modify the file
        fs::write(&makefile_path, "test:\n\techo testing").unwrap();

        // Cache should now miss (content changed)
        assert!(cache.get(&makefile_path).is_none());
    }

    #[test]
    fn test_cache_prune() {
        let mut cache = Cache::new();

        // Add entry for non-existent file
        let fake_path = "/nonexistent/Makefile".to_string();
        cache.entries.insert(
            fake_path.clone(),
            CacheEntry {
                content_hash: "abc123".to_string(),
                modified_time: 0,
                targets: vec![],
            },
        );

        assert_eq!(cache.entries.len(), 1);

        // Prune should remove the entry
        cache.prune();

        assert!(cache.entries.is_empty());
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = Cache::new();

        cache.entries.insert(
            "test".to_string(),
            CacheEntry {
                content_hash: "abc".to_string(),
                modified_time: 0,
                targets: vec![],
            },
        );

        assert!(!cache.entries.is_empty());

        cache.clear();

        assert!(cache.entries.is_empty());
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = Cache::new();

        cache.entries.insert(
            "file1".to_string(),
            CacheEntry {
                content_hash: "abc".to_string(),
                modified_time: 0,
                targets: vec![
                    Target::new("a".to_string(), None, PathBuf::from("f"), 1),
                    Target::new("b".to_string(), None, PathBuf::from("f"), 2),
                ],
            },
        );

        cache.entries.insert(
            "file2".to_string(),
            CacheEntry {
                content_hash: "def".to_string(),
                modified_time: 0,
                targets: vec![Target::new("c".to_string(), None, PathBuf::from("f"), 1)],
            },
        );

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 2);
        assert_eq!(stats.total_targets, 3);
    }

    #[test]
    fn test_cache_serialization() {
        let mut cache = Cache::new();

        cache.entries.insert(
            "/test/Makefile".to_string(),
            CacheEntry {
                content_hash: "abc123".to_string(),
                modified_time: 1234567890,
                targets: vec![Target::new(
                    "build".to_string(),
                    Some("Build it".to_string()),
                    PathBuf::from("/test/Makefile"),
                    1,
                )],
            },
        );

        // Serialize
        let json = serde_json::to_string(&cache).unwrap();

        // Deserialize
        let loaded: Cache = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.version, cache.version);
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entries.contains_key("/test/Makefile"));
    }
}
