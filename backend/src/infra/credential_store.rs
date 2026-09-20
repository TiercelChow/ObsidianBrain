use crate::error::BrainError;

const SERVICE_NAME: &str = "ObsidianBrain LLM Providers";

pub trait ProviderCredentialStore: Send + Sync {
    fn get(&self, provider_id: &str) -> Result<Option<String>, BrainError>;
    fn set(&self, provider_id: &str, secret: &str) -> Result<(), BrainError>;
    fn delete(&self, provider_id: &str) -> Result<(), BrainError>;
}

#[derive(Clone, Default)]
pub struct SystemProviderCredentialStore;

impl SystemProviderCredentialStore {
    fn entry(provider_id: &str) -> Result<keyring::Entry, BrainError> {
        keyring::Entry::new(SERVICE_NAME, provider_id).map_err(credential_error)
    }
}

impl ProviderCredentialStore for SystemProviderCredentialStore {
    fn get(&self, provider_id: &str) -> Result<Option<String>, BrainError> {
        match Self::entry(provider_id)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(credential_error(error)),
        }
    }

    fn set(&self, provider_id: &str, secret: &str) -> Result<(), BrainError> {
        if secret.trim().is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "API Key 不能为空".to_string(),
            ));
        }
        Self::entry(provider_id)?
            .set_password(secret)
            .map_err(credential_error)
    }

    fn delete(&self, provider_id: &str) -> Result<(), BrainError> {
        match Self::entry(provider_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(credential_error(error)),
        }
    }
}

fn credential_error(error: keyring::Error) -> BrainError {
    BrainError::Internal(format!("系统凭据库操作失败: {error}"))
}

#[cfg(test)]
pub mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

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
                .insert(provider_id.to_string(), secret.to_string());
            Ok(())
        }

        fn delete(&self, provider_id: &str) -> Result<(), BrainError> {
            self.values.lock().unwrap().remove(provider_id);
            Ok(())
        }
    }

    #[test]
    fn test_memory_provider_credentials_never_expose_missing_values() {
        let store = MemoryProviderCredentialStore::default();
        assert_eq!(store.get("provider-a").unwrap(), None);
        store.set("provider-a", "secret-value").unwrap();
        assert_eq!(
            store.get("provider-a").unwrap().as_deref(),
            Some("secret-value")
        );
        store.delete("provider-a").unwrap();
        assert_eq!(store.get("provider-a").unwrap(), None);
    }
}
