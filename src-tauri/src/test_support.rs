use crate::*;

#[cfg(test)]
pub(crate) fn runtime_refresh_token_cache_clear() {
    if let Ok(mut guard) = runtime_refresh_token_cache().lock() {
        guard.clear();
    }
}

#[cfg(test)]
fn test_token_keyring_store() -> &'static Mutex<HashMap<(String, String), String>> {
    use std::sync::OnceLock;
    static STORE: OnceLock<Mutex<HashMap<(String, String), String>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn test_token_keyring_available_flag() -> &'static std::sync::atomic::AtomicBool {
    use std::sync::atomic::AtomicBool;
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<AtomicBool> = OnceLock::new();
    AVAILABLE.get_or_init(|| AtomicBool::new(true))
}

#[cfg(test)]
fn test_token_keyring_read_fail_services() -> &'static Mutex<HashSet<String>> {
    use std::sync::OnceLock;
    static SERVICES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SERVICES.get_or_init(|| Mutex::new(HashSet::new()))
}

#[cfg(test)]
fn test_token_keyring_available() -> bool {
    use std::sync::atomic::Ordering;
    test_token_keyring_available_flag().load(Ordering::SeqCst)
}

#[cfg(test)]
pub(crate) fn set_test_token_keyring_available(value: bool) {
    use std::sync::atomic::Ordering;
    test_token_keyring_available_flag().store(value, Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn set_test_token_keyring_read_failure(service: &str, should_fail: bool) {
    if let Ok(mut guard) = test_token_keyring_read_fail_services().lock() {
        if should_fail {
            guard.insert(service.to_string());
        } else {
            guard.remove(service);
        }
    }
}

#[cfg(test)]
pub(crate) fn clear_test_token_keyring_store() {
    if let Ok(mut guard) = test_token_keyring_store().lock() {
        guard.clear();
    }
    if let Ok(mut guard) = test_token_keyring_read_fail_services().lock() {
        guard.clear();
    }
    runtime_refresh_token_cache_clear();
}

#[cfg(test)]
pub(crate) fn test_secure_storage_guard() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    let guard: MutexGuard<'static, ()> = GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("secure-storage test mutex lock");
    guard
}

#[cfg(test)]
pub(crate) fn token_keyring_set_secret(
    service: &str,
    username: &str,
    secret: &str,
) -> Result<(), String> {
    if !test_token_keyring_available() {
        return Err(format!(
            "keyring write failed: {}",
            keyring_unavailable_hint()
        ));
    }
    let mut guard = test_token_keyring_store()
        .lock()
        .map_err(|_| "test keyring lock failed".to_string())?;
    guard.insert(
        (service.to_string(), username.to_string()),
        secret.to_string(),
    );
    Ok(())
}

#[cfg(test)]
pub(crate) fn token_keyring_get_secret(
    service: &str,
    username: &str,
) -> Result<Option<String>, String> {
    if !test_token_keyring_available() {
        return Err(format!(
            "keyring read failed: {}",
            keyring_unavailable_hint()
        ));
    }
    let should_fail = test_token_keyring_read_fail_services()
        .lock()
        .map_err(|_| "test keyring lock failed".to_string())?
        .contains(service);
    if should_fail {
        return Err(format!(
            "keyring read failed: simulated read failure for service '{service}'"
        ));
    }
    let guard = test_token_keyring_store()
        .lock()
        .map_err(|_| "test keyring lock failed".to_string())?;
    Ok(guard
        .get(&(service.to_string(), username.to_string()))
        .cloned())
}

#[cfg(test)]
pub(crate) fn token_keyring_delete_secret(service: &str, username: &str) -> Result<(), String> {
    if !test_token_keyring_available() {
        return Err(format!(
            "keyring delete failed: {}",
            keyring_unavailable_hint()
        ));
    }
    let mut guard = test_token_keyring_store()
        .lock()
        .map_err(|_| "test keyring lock failed".to_string())?;
    guard.remove(&(service.to_string(), username.to_string()));
    Ok(())
}
