use std::{collections::HashMap, fmt::Display};

pub struct Entry<T: Display> {
    value: T,
}

impl<T> Entry<T>
where
    T: Display,
{
    fn from(value: T) -> Self {
        Self { value }
    }

    fn get(&self) -> Option<&T> {
        Some(&self.value)
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

    pub fn insert(&mut self, key: String, value: T) -> Option<T> {
        self.map
            .insert(key, Entry::from(value))
            .map(|val| val.value)
    }
}
