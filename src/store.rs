use std::{collections::HashMap, fmt::Display};

pub struct Entry<T: Display> {
    value: T,
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
        self.map.get(key).map(|val| &val.value)
    }
}
