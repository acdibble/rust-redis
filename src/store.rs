use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    command::{SetExpirationMode, SetInsertionMode},
    value::Value,
};

fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis()
}

#[derive(Debug)]
pub struct Entry {
    value: Value,
    expires_at: Option<u128>,
}

impl Entry {
    fn from(value: Value) -> Self {
        Self {
            value,
            expires_at: None,
        }
    }

    fn get(&self) -> Option<&Value> {
        match self.expires_at {
            Some(time) if time < now() => None,
            _ => Some(&self.value),
        }
    }

    fn consume(self) -> Option<Value> {
        match self.expires_at {
            Some(time) if time < now() => None,
            _ => Some(self.value),
        }
    }
}

impl std::fmt::Display for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

pub struct Store {
    map: HashMap<String, Entry>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self.map.get(key) {
            Some(value) => value.get(),
            None => None,
        }
    }

    pub fn has_entry(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn insert(
        &mut self,
        key: String,
        value: Value,
        insertion_mode: SetInsertionMode,
        expiration_mode: SetExpirationMode,
    ) -> Result<Option<Value>, ()> {
        let mut new_entry = Entry::from(value);
        let current = self.map.get(&key);

        match expiration_mode {
            SetExpirationMode::Normal => {}
            SetExpirationMode::ExpiryMilliseconds(expiry) => {
                new_entry.expires_at = Some(expiry + now())
            }
            SetExpirationMode::ExpirySeconds(expiry) => {
                new_entry.expires_at = Some(expiry * 1000 + now())
            }
            SetExpirationMode::ExpiryUTCMilliseconds(expiry) => new_entry.expires_at = Some(expiry),
            SetExpirationMode::ExpiryUTCSeconds(expiry) => {
                new_entry.expires_at = Some(expiry * 1000)
            }
            SetExpirationMode::KeepTTL => {
                new_entry.expires_at = match current {
                    Some(Entry {
                        expires_at: Some(previous_expiry),
                        ..
                    }) => Some(*previous_expiry),
                    _ => None,
                }
            }
        }

        let entry_exists = self.has_entry(&key);

        let previous = match insertion_mode {
            SetInsertionMode::Normal => Ok(self.map.insert(key, new_entry)),
            SetInsertionMode::IfNotExists if !entry_exists => Ok(self.map.insert(key, new_entry)),
            SetInsertionMode::IfExists if entry_exists => Ok(self.map.insert(key, new_entry)),
            _ => Err(()),
        };

        match previous {
            Ok(result) => match result {
                Some(entry) => Ok(entry.consume()),
                None => Ok(None),
            },
            Err(()) => Err(()),
        }
    }
}
