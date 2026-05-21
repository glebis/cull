use std::collections::HashMap;
use std::sync::Mutex;

pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn delete(&self, key: &str) -> Result<(), String>;
}

pub struct KeychainStore {
    service: String,
}

impl KeychainStore {
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_string(),
        }
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(&self.service, key).map_err(|e| format!("keyring error: {}", e))
    }
}

impl SecretStore for KeychainStore {
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.entry(key)?
            .set_password(value)
            .map_err(|e| format!("Failed to store in OS keychain: {}", e))
    }

    fn get(&self, key: &str) -> Result<Option<String>, String> {
        match self.entry(key)?.get_password() {
            Ok(val) => Ok(Some(val)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("Failed to read from OS keychain: {}", e)),
        }
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        match self.entry(key)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Failed to delete from OS keychain: {}", e)),
        }
    }
}

#[allow(dead_code)]
pub struct MemoryStore {
    data: Mutex<HashMap<String, String>>,
}

#[allow(dead_code)]
impl MemoryStore {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }
}

impl SecretStore for MemoryStore {
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.data
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_set_get() {
        let store = MemoryStore::new();
        assert_eq!(store.get("api_key_google").unwrap(), None);

        store.set("api_key_google", "test_google_key_123").unwrap();
        assert_eq!(
            store.get("api_key_google").unwrap(),
            Some("test_google_key_123".to_string())
        );
    }

    #[test]
    fn memory_store_overwrite() {
        let store = MemoryStore::new();
        store.set("api_key_google", "key_v1").unwrap();
        store.set("api_key_google", "key_v2").unwrap();
        assert_eq!(
            store.get("api_key_google").unwrap(),
            Some("key_v2".to_string())
        );
    }

    #[test]
    fn memory_store_delete() {
        let store = MemoryStore::new();
        store.set("api_key_google", "key_to_delete").unwrap();
        store.delete("api_key_google").unwrap();
        assert_eq!(store.get("api_key_google").unwrap(), None);
    }

    #[test]
    fn memory_store_delete_nonexistent() {
        let store = MemoryStore::new();
        assert!(store.delete("nonexistent").is_ok());
    }

    #[test]
    fn memory_store_multiple_providers() {
        let store = MemoryStore::new();
        store.set("api_key_google", "google_key").unwrap();
        store.set("api_key_openai", "openai_key").unwrap();

        assert_eq!(
            store.get("api_key_google").unwrap(),
            Some("google_key".to_string())
        );
        assert_eq!(
            store.get("api_key_openai").unwrap(),
            Some("openai_key".to_string())
        );
    }
}
