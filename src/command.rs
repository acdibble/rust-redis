use crate::value::Value;
use std::vec::IntoIter;

#[derive(Debug)]
pub enum SetInsertionMode {
    Normal,
    NX,
    XX,
}

#[derive(Debug)]
pub enum SetExpirationMode {
    Normal,
    // Ex(u128),
    Px(u128),
    // Exat,
    // Pxat,
    KeepTTL,
}

impl SetExpirationMode {
    fn from(string: &str, amount: u128) -> Self {
        match string {
            // "EX" => Self::Ex(amount),
            "PX" => Self::Px(amount),
            // "EXAT" => Self::Exat,
            // "PXAT" => Self::Pxat,
            // "KEEPTTL" => Self::KeepTTL,
            _ => Self::Normal,
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Ping(Option<Value>),
    Echo(Value),
    Get(Value),
    Set {
        key: Value,
        value: Value,
        insertion_mode: SetInsertionMode,
        expiration_mode: SetExpirationMode,
        return_mode: bool,
    },
    Error(Value),
}

impl Command {
    fn build_error(message: &'static str) -> Self {
        Self::Error(Value::StaticError(message))
    }

    fn parse_integer(value: String) -> std::result::Result<u128, Self> {
        match value.parse::<u128>() {
            Ok(num) => Ok(num),
            Err(_) => Err(Command::build_error(
                "value is not an integer or out of range",
            )),
        }
    }

    fn parse_set(mut values: IntoIter<Value>) -> Option<Self> {
        if values.len() < 2 {
            return None;
        }

        let key = values.next().unwrap();
        let value = values.next().unwrap();
        let mut insertion_mode = SetInsertionMode::Normal;
        let mut expiration_mode = SetExpirationMode::Normal;
        let mut return_mode = false;

        while let Some(Value::BulkString(value)) = values.next() {
            match value.as_str() {
                "XX" if matches!(insertion_mode, SetInsertionMode::Normal) => {
                    insertion_mode = SetInsertionMode::XX
                }
                "NX" if matches!(insertion_mode, SetInsertionMode::Normal) => {
                    insertion_mode = SetInsertionMode::NX
                }
                kind @ ("EX" | "PX" | "EXAT" | "PXAT")
                    if matches!(expiration_mode, SetExpirationMode::Normal) =>
                {
                    match values.next() {
                        Some(Value::BulkString(value)) => match Command::parse_integer(value) {
                            Ok(num) => expiration_mode = SetExpirationMode::from(kind, num),
                            Err(cmd) => return Some(cmd),
                        },
                        _ => return Some(Command::build_error("syntax error")),
                    }
                }
                "KEEPTTL" if matches!(expiration_mode, SetExpirationMode::Normal) => {
                    expiration_mode = SetExpirationMode::KeepTTL
                }
                "GET" => return_mode = true,
                _ => return Some(Command::build_error("syntax error")),
            }
        }

        Some(Self::Set {
            key,
            value,
            insertion_mode,
            expiration_mode,
            return_mode,
        })
    }
}

impl TryFrom<Value> for Command {
    type Error = tokio::io::Error;

    fn try_from(value: Value) -> std::result::Result<Self, <Self as TryFrom<Value>>::Error> {
        let mut values = value.into_array()?.into_iter();
        let command = values.next().unwrap();

        let cmd = match command.as_str()? {
            "PING" | "ping" => match (values.next(), values.len()) {
                (message, 0) => Some(Self::Ping(message)),
                _ => None,
            },
            "ECHO" | "echo" => match (values.next(), values.len()) {
                (Some(message), 0) => Some(Self::Echo(message)),
                _ => None,
            },
            "GET" | "get" => match (values.next(), values.len()) {
                (Some(key), 0) => Some(Self::Get(key)),
                _ => None,
            },
            "SET" | "set" => Command::parse_set(values),
            string => Some(Self::Error(Value::Error(format!(
                "unknown command '{}'",
                string
            )))),
        };

        match cmd {
            Some(result) => Ok(result),
            None => Ok(Self::Error(Value::StaticError(
                "ERR wrong number of arguments for command",
            ))),
        }
    }
}
