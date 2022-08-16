use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::value::Value;

fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis()
}

#[derive(Debug)]
pub enum InsertionMode {
    Normal,
    IfNotExists,
    IfExists,
}

impl InsertionMode {
    pub fn is_normal(&self) -> bool {
        matches!(self, InsertionMode::Normal)
    }
}

#[derive(Debug)]
pub enum ExpirationMode {
    Normal,
    ExpirySeconds(u128),
    ExpiryMilliseconds(u128),
    ExpiryUTCSeconds(u128),
    ExpiryUTCMilliseconds(u128),
    KeepTTL,
}

impl ExpirationMode {
    pub fn is_normal(&self) -> bool {
        matches!(self, ExpirationMode::Normal)
    }

    pub fn from(string: &str, amount: u128) -> Self {
        match string {
            "EX" | "ex" => Self::ExpirySeconds(amount),
            "PX" | "px" => Self::ExpiryMilliseconds(amount),
            "EXAT" | "exat" => Self::ExpiryUTCSeconds(amount),
            "PXAT" | "pxat" => Self::ExpiryUTCMilliseconds(amount),
            "KEEPTTL" | "keepttl" => Self::KeepTTL,
            _ => Self::Normal,
        }
    }
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

    fn is_valid(&self) -> bool {
        match self.expires_at {
            Some(time) => time > now(),
            _ => true,
        }
    }

    fn get(&self) -> Option<&Value> {
        if self.is_valid() {
            Some(&self.value)
        } else {
            None
        }
    }

    fn consume(self) -> Option<Value> {
        if self.is_valid() {
            Some(self.value)
        } else {
            None
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
        insertion_mode: InsertionMode,
        expiration_mode: ExpirationMode,
    ) -> Result<Option<Value>, ()> {
        let mut new_entry = Entry::from(value);
        let current = self.map.get(&key);

        match expiration_mode {
            ExpirationMode::Normal => {}
            ExpirationMode::ExpiryMilliseconds(expiry) => {
                new_entry.expires_at = Some(expiry + now())
            }
            ExpirationMode::ExpirySeconds(expiry) => {
                new_entry.expires_at = Some(expiry * 1000 + now())
            }
            ExpirationMode::ExpiryUTCMilliseconds(expiry) => new_entry.expires_at = Some(expiry),
            ExpirationMode::ExpiryUTCSeconds(expiry) => new_entry.expires_at = Some(expiry * 1000),
            ExpirationMode::KeepTTL => {
                new_entry.expires_at = match current {
                    Some(Entry {
                        expires_at: Some(previous_expiry),
                        ..
                    }) => Some(*previous_expiry),
                    _ => None,
                }
            }
        }

        let entry_exists = match current {
            Some(entry) => entry.is_valid(),
            _ => false,
        };

        let previous = match insertion_mode {
            InsertionMode::Normal => Ok(self.map.insert(key, new_entry)),
            InsertionMode::IfNotExists if !entry_exists => Ok(self.map.insert(key, new_entry)),
            InsertionMode::IfExists if entry_exists => Ok(self.map.insert(key, new_entry)),
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
