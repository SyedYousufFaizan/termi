//! Metadata caching for VFS
//!
//! SAF operations are slow (~50-200ms each). This cache stores metadata
//! to avoid repeated ContentResolver calls.

use super::provider::FileMetadata;
use log::{debug, trace};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Cache entry with TTL
struct CacheEntry {
    metadata: FileMetadata,
    inserted_at: Instant,
}

/// Metadata cache for VFS operations
pub struct MetadataCache {
    /// Cached entries
    entries: HashMap<PathBuf, CacheEntry>,
    /// Time-to-live for cache entries
    ttl: Duration,
    /// Maximum number of entries
    max_entries: usize,
    /// Cache statistics
    stats: CacheStats,
}

/// Cache statistics for monitoring
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub insertions: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl MetadataCache {
    /// Create a new cache with default settings
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            ttl: Duration::from_secs(5),
            max_entries: 10000,
            stats: CacheStats::default(),
        }
    }

    /// Create with custom TTL
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            ttl,
            ..Self::new()
        }
    }

    /// Create with custom capacity
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Self::new()
        }
    }

    /// Get metadata from cache
    pub fn get(&mut self, path: &Path) -> Option<FileMetadata> {
        let key = path.to_path_buf();
        
        // First check if entry exists and is valid
        let (valid, expired) = match self.entries.get(&key) {
            Some(entry) => {
                if entry.inserted_at.elapsed() < self.ttl {
                    (true, false)
                } else {
                    (false, true)
                }
            }
            None => (false, false),
        };

        if expired {
            self.entries.remove(&key);
            self.stats.evictions += 1;
        }

        if valid {
            self.stats.hits += 1;
            trace!("Cache hit for {}", path.display());
            // Re-fetch after removing the immutable borrow
            self.entries.get(&key).map(|e| e.metadata.clone())
        } else {
            self.stats.misses += 1;
            trace!("Cache miss for {}", path.display());
            None
        }
    }

    /// Insert metadata into cache
    pub fn insert(&mut self, path: &Path, metadata: FileMetadata) {
        // Evict old entries if at capacity
        if self.entries.len() >= self.max_entries {
            self.evict_oldest();
        }

        self.entries.insert(
            path.to_path_buf(),
            CacheEntry {
                metadata,
                inserted_at: Instant::now(),
            },
        );
        
        self.stats.insertions += 1;
        trace!("Cached metadata for {}", path.display());
    }

    /// Remove entry from cache
    pub fn invalidate(&mut self, path: &Path) {
        self.entries.remove(path);
        debug!("Invalidated cache for {}", path.display());
    }

    /// Invalidate all entries under a path (for directory operations)
    pub fn invalidate_prefix(&mut self, prefix: &Path) {
        let keys_to_remove: Vec<_> = self.entries
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        
        for key in keys_to_remove {
            self.entries.remove(&key);
        }
        
        debug!("Invalidated {} entries under {}", self.entries.len(), prefix.display());
    }

    /// Clear the entire cache
    pub fn clear(&mut self) {
        self.entries.clear();
        debug!("Cache cleared");
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Evict oldest entry
    fn evict_oldest(&mut self) {
        if let Some(oldest) = self.entries
            .iter()
            .min_by_key(|(_, entry)| entry.inserted_at)
            .map(|(k, _)| k.clone())
        {
            self.entries.remove(&oldest);
            self.stats.evictions += 1;
        }
    }

    /// Evict all expired entries
    pub fn evict_expired(&mut self) -> usize {
        let now = Instant::now();
        let expired: Vec<_> = self.entries
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.inserted_at) >= self.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        
        let count = expired.len();
        for key in expired {
            self.entries.remove(&key);
            self.stats.evictions += 1;
        }
        
        count
    }
}

impl Default for MetadataCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let mut cache = MetadataCache::new();
        
        let meta = FileMetadata::file("test.txt", "/test.txt", 100);
        cache.insert(Path::new("/test.txt"), meta.clone());
        
        let cached = cache.get(Path::new("/test.txt"));
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().size, 100);
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = MetadataCache::new();
        assert!(cache.get(Path::new("/nonexistent")).is_none());
    }

    #[test]
    fn test_cache_invalidation() {
        let mut cache = MetadataCache::new();
        
        cache.insert(Path::new("/a/b.txt"), FileMetadata::file("b.txt", "/a/b.txt", 10));
        cache.insert(Path::new("/a/c.txt"), FileMetadata::file("c.txt", "/a/c.txt", 20));
        cache.insert(Path::new("/d.txt"), FileMetadata::file("d.txt", "/d.txt", 30));
        
        cache.invalidate_prefix(Path::new("/a"));
        
        assert!(cache.get(Path::new("/a/b.txt")).is_none());
        assert!(cache.get(Path::new("/a/c.txt")).is_none());
        assert!(cache.get(Path::new("/d.txt")).is_some());
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = MetadataCache::new();
        
        cache.insert(Path::new("/test"), FileMetadata::file("test", "/test", 0));
        
        // Hit
        cache.get(Path::new("/test"));
        // Miss
        cache.get(Path::new("/other"));
        
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.insertions, 1);
    }

    #[test]
    fn test_cache_expiry() {
        let mut cache = MetadataCache::with_ttl(Duration::from_millis(10));
        
        cache.insert(Path::new("/test"), FileMetadata::file("test", "/test", 0));
        assert!(cache.get(Path::new("/test")).is_some());
        
        std::thread::sleep(Duration::from_millis(20));
        assert!(cache.get(Path::new("/test")).is_none());
    }
}
