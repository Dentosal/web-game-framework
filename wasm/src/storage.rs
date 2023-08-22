//! Local storage utilities

use serde::{de::DeserializeOwned, Serialize};

pub fn get(key: &str) -> Option<String> {
    web_sys::window()
        .unwrap()
        .local_storage()
        .unwrap()
        .unwrap()
        .get_item(key)
        .unwrap()
}

pub fn set(key: &str, value: &str) {
    web_sys::window()
        .unwrap()
        .local_storage()
        .unwrap()
        .unwrap()
        .set_item(key, value)
        .unwrap();
}

/// Returns `None` on both missing keys and invalid values
pub fn get_typed<T: DeserializeOwned>(key: &str) -> Option<T> {
    let raw_value = get(key)?;
    serde_json::from_str(&raw_value).ok()?
}

pub fn set_typed<T: Serialize>(key: &str, value: &T) {
    let value = serde_json::to_string(value).expect("Unable to serialize value");
    set(key, &value);
}
