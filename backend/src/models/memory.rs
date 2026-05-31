use serde::{Deserialize, Serialize};

/// Aggregated statistics about the vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total number of markdown files in the vault.
    pub total_files: usize,
    /// Path to the vault directory.
    pub vault_path: String,
    /// Name of the vault.
    pub vault_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stats_roundtrip() {
        let stats = MemoryStats {
            total_files: 128,
            vault_path: "/path/to/vault".to_string(),
            vault_name: "MyVault".to_string(),
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: MemoryStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_files, stats.total_files);
        assert_eq!(parsed.vault_path, stats.vault_path);
        assert_eq!(parsed.vault_name, stats.vault_name);
    }

    #[test]
    fn test_memory_stats_empty() {
        let stats = MemoryStats {
            total_files: 0,
            vault_path: "".to_string(),
            vault_name: "".to_string(),
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: MemoryStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_files, 0);
    }
}
