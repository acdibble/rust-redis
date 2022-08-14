use std::{
    collections::HashMap,
    fmt::Display,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::command::{SetExpirationMode, SetInsertionMode};

fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis()
}

pub struct Entry<T: Display> {
    value: T,
    expires_at: Option<u128>,
}

impl<T> Entry<T>
where
    T: Display,
{
    fn from(value: T) -> Self {
        Self {
            value,
            expires_at: None,
        }
    }

    fn get(&self) -> Option<&T> {
        match self.expires_at {
            Some(time) if time < now() => None,
            _ => Some(&self.value),
        }
    }

    fn consume(self) -> Option<T> {
        match self.expires_at {
            Some(time) if time < now() => None,
            _ => Some(self.value),
        }
    }
}

impl<T> std::fmt::Display for Entry<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

pub struct Store<T: Display> {
    map: HashMap<String, Entry<T>>,
}

impl<T: Display> Store<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        match self.map.get(key) {
            Some(value) => value.get(),
            None => None,
        }
    }

    pub fn insert(
        &mut self,
        key: String,
        value: T,
        insertion_mode: SetInsertionMode,
        expiration_mode: SetExpirationMode,
    ) -> Option<T> {
        let mut new_entry = Entry::from(value);

        if let SetExpirationMode::Px(expiry) = expiration_mode {
            new_entry.expires_at = Some(expiry + now());
        }

        let previous = match insertion_mode {
            SetInsertionMode::Normal => self.map.insert(key, new_entry),
            _ => todo!(),
        };

        // self.map.insert(k, v)
        if let Some(previous) = previous {
            previous.consume()
        } else {
            None
        }
    }
}
