use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::Mutex as AsyncMutex;
use zeroize::Zeroizing;

use crate::error::BrainError;

pub(crate) const LOCAL_FILE: &str = "local_file";
pub(crate) type ProviderSecret = Arc<Zeroizing<String>>;

pub trait ProviderCredentialStore: Send + Sync {
    fn get(&self, provider_id: &str) -> Result<Option<String>, BrainError>;
    fn set(&self, provider_id: &str, secret: &str) -> Result<(), BrainError>;
    fn delete(&self, provider_id: &str) -> Result<(), BrainError>;
}

pub struct LocalProviderCredentialStore {
    path: PathBuf,
}

impl LocalProviderCredentialStore {
    pub fn for_database(database: &Path) -> Self {
        let path = if database.file_name().is_some_and(|name| name == "brain.db") {
            database.with_file_name("model-credentials.json")
        } else {
            database.with_extension("model-credentials.json")
        };
        Self { path }
    }

    fn load(&self) -> Result<BTreeMap<String, String>, BrainError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => Zeroizing::new(bytes),
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(error) => return Err(error.into()),
        };
        serde_json::from_slice(&bytes).map_err(|_| {
            BrainError::KnowledgeValidation(format!(
                "模型凭据文件格式有误，请检查 {}",
                self.path.display()
            ))
        })
    }

    fn update(&self, provider_id: &str, secret: Option<&str>) -> Result<(), BrainError> {
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // The lock stays in place when the JSON file is atomically replaced.
        let lock = options.open(self.path.with_extension("lock"))?;
        lock.lock()?;
        let mut entries = self.load()?;
        match secret {
            Some(secret) => {
                entries.insert(provider_id.to_owned(), secret.to_owned());
            }
            None if entries.remove(provider_id).is_none() => return Ok(()),
            None => {}
        }
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            pending
                .as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        serde_json::to_writer_pretty(&mut pending, &entries)
            .map_err(|_| BrainError::Internal("无法写入模型凭据文件".into()))?;
        pending.write_all(b"\n")?;
        pending.as_file().sync_all()?;
        pending.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

impl ProviderCredentialStore for LocalProviderCredentialStore {
    fn get(&self, provider_id: &str) -> Result<Option<String>, BrainError> {
        Ok(self
            .load()?
            .remove(provider_id)
            .filter(|secret| !secret.trim().is_empty()))
    }

    fn set(&self, provider_id: &str, secret: &str) -> Result<(), BrainError> {
        if secret.trim().is_empty() {
            return Err(BrainError::KnowledgeValidation("API Key 不能为空".into()));
        }
        self.update(provider_id, Some(secret))
    }

    fn delete(&self, provider_id: &str) -> Result<(), BrainError> {
        self.update(provider_id, None)
    }
}

pub(crate) struct ProcessCredentials {
    store: Arc<dyn ProviderCredentialStore>,
    entries: Mutex<HashMap<String, Arc<AsyncMutex<ProviderCredentials>>>>,
}

impl ProcessCredentials {
    pub(crate) fn new(store: Arc<dyn ProviderCredentialStore>) -> Self {
        Self {
            store,
            entries: Mutex::default(),
        }
    }

    pub(crate) fn provider(
        &self,
        id: &str,
    ) -> Result<Arc<AsyncMutex<ProviderCredentials>>, BrainError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| BrainError::Internal("模型凭据暂不可用，请重启本地服务".into()))?;
        Ok(entries
            .entry(id.into())
            .or_insert_with(|| {
                Arc::new(AsyncMutex::new(ProviderCredentials {
                    id: id.into(),
                    store: self.store.clone(),
                    cached: None,
                }))
            })
            .clone())
    }
}

pub(crate) struct ProviderCredentials {
    id: String,
    store: Arc<dyn ProviderCredentialStore>,
    cached: Option<(i64, ProviderSecret)>,
}

impl ProviderCredentials {
    pub(crate) async fn read(
        &mut self,
        revision: i64,
    ) -> Result<Option<ProviderSecret>, BrainError> {
        if let Some((_, secret)) = self.cached.as_ref().filter(|(saved, _)| *saved == revision) {
            return Ok(Some(secret.clone()));
        }
        let store = self.store.clone();
        let id = self.id.clone();
        let secret = tokio::task::spawn_blocking(move || store.get(&id))
            .await
            .map_err(|_| BrainError::Internal("读取模型凭据的进程内任务意外结束".into()))??
            .filter(|value| !value.trim().is_empty())
            .map(|value| Arc::new(Zeroizing::new(value)));
        self.remember(revision, secret.clone());
        Ok(secret)
    }

    async fn write(&mut self, secret: Option<ProviderSecret>) -> Result<(), BrainError> {
        self.cached = None;
        let store = self.store.clone();
        let id = self.id.clone();
        tokio::task::spawn_blocking(move || match secret {
            Some(secret) => store.set(&id, secret.as_str()),
            None => store.delete(&id),
        })
        .await
        .map_err(|_| BrainError::Internal("写入模型凭据的进程内任务意外结束".into()))?
    }

    pub(crate) async fn update<R>(
        &mut self,
        previous: Option<ProviderSecret>,
        next: Option<ProviderSecret>,
        save_metadata: impl FnOnce() -> Result<R, BrainError>,
    ) -> Result<R, BrainError> {
        let changed =
            previous.as_ref().map(|key| key.as_str()) != next.as_ref().map(|key| key.as_str());
        if changed {
            self.write(next).await?;
        }
        let result = save_metadata();
        if changed && result.is_err() {
            self.write(previous).await.map_err(|error| {
                BrainError::Internal(format!(
                    "保存供应商配置失败，恢复原有模型凭据也失败：{error}"
                ))
            })?;
        }
        result
    }

    pub(crate) fn remember(&mut self, revision: i64, secret: Option<ProviderSecret>) {
        self.cached = secret.map(|value| (revision, value));
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[derive(Default)]
    pub struct MemoryProviderCredentialStore {
        values: Mutex<HashMap<String, String>>,
    }

    impl ProviderCredentialStore for MemoryProviderCredentialStore {
        fn get(&self, provider_id: &str) -> Result<Option<String>, BrainError> {
            Ok(self.values.lock().unwrap().get(provider_id).cloned())
        }

        fn set(&self, provider_id: &str, secret: &str) -> Result<(), BrainError> {
            self.values
                .lock()
                .unwrap()
                .insert(provider_id.into(), secret.into());
            Ok(())
        }

        fn delete(&self, provider_id: &str) -> Result<(), BrainError> {
            self.values.lock().unwrap().remove(provider_id);
            Ok(())
        }
    }

    #[test]
    fn test_local_credential_store_keeps_one_file_and_removes_only_the_selected_key() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("brain.db");
        let store = LocalProviderCredentialStore::for_database(&database);
        assert!(store.get("one").unwrap().is_none());
        std::thread::scope(|scope| {
            scope.spawn(|| store.set("one", "test-key-one").unwrap());
            scope.spawn(|| store.set("two", "test-key-two").unwrap());
        });
        assert_eq!(store.get("one").unwrap().as_deref(), Some("test-key-one"));
        assert_eq!(store.get("two").unwrap().as_deref(), Some("test-key-two"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&store.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let reopened = LocalProviderCredentialStore::for_database(&database);
        reopened.delete("one").unwrap();
        assert!(store.get("one").unwrap().is_none());
        assert_eq!(store.get("two").unwrap().as_deref(), Some("test-key-two"));
    }

    #[test]
    fn test_local_credential_store_preserves_malformed_files_and_hides_their_content() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalProviderCredentialStore::for_database(&dir.path().join("brain.db"));
        fs::write(&store.path, "\"malformed-private-value\"").unwrap();
        let error = store.set("one", "test-key").unwrap_err().to_string();
        assert!(error.contains("模型凭据文件格式有误"));
        assert!(!error.contains("malformed-private-value"));
        assert_eq!(
            fs::read_to_string(&store.path).unwrap(),
            "\"malformed-private-value\""
        );
    }

    #[tokio::test]
    async fn test_credential_update_restores_previous_value_if_profile_write_fails() {
        let store = Arc::new(MemoryProviderCredentialStore::default());
        store.set("provider", "original").unwrap();
        let credentials = ProcessCredentials::new(store.clone());
        let slot = credentials.provider("provider").unwrap();
        let mut credential = slot.lock().await;
        let previous = credential.read(1).await.unwrap();
        let result = credential
            .update(
                previous,
                Some(Arc::new(Zeroizing::new("next".into()))),
                || Err::<(), _>(BrainError::KnowledgeConflict("配置已变化".into())),
            )
            .await;
        assert!(matches!(result, Err(BrainError::KnowledgeConflict(_))));
        assert_eq!(store.get("provider").unwrap().as_deref(), Some("original"));
    }
}
