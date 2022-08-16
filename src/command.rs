use crate::store::{ExpirationMode, InsertionMode};
use crate::value::Value;
use std::vec::IntoIter;

#[derive(Debug)]
pub enum Command {
    Ping(Option<Value>),
    Echo(Value),
    Get(Value),
    Set {
        key: Value,
        value: Value,
        insertion_mode: InsertionMode,
        expiration_mode: ExpirationMode,
        return_mode: bool,
    },
    Exists(Vec<Value>),
    Error(Value),
}

impl Command {
    fn build_error(message: &'static str) -> Self {
        Self::Error(Value::StaticError(message))
    }

    fn parse_integer(value: String) -> std::result::Result<i64, Self> {
        match value.parse::<i64>() {
            Ok(num) => Ok(num),
            Err(_) => Err(Command::build_error(
                "value is not an integer or out of range",
            )),
        }
    }

    fn parse_set_command(mut values: IntoIter<Value>) -> Option<Self> {
        if values.len() < 2 {
            return None;
        }

        let key = values.next().unwrap();
        let value = values.next().unwrap();
        let mut insertion_mode = InsertionMode::Normal;
        let mut expiration_mode = ExpirationMode::Normal;
        let mut return_mode = false;

        while let Some(Value::BulkString(value)) = values.next() {
            println!("arg: {value:?}, {}", values.len());
            match value.as_str() {
                "XX" | "xx" if insertion_mode.is_normal() => {
                    insertion_mode = InsertionMode::IfExists
                }
                "NX" | "nx" if insertion_mode.is_normal() => {
                    insertion_mode = InsertionMode::IfNotExists
                }
                kind @ ("EX" | "PX" | "EXAT" | "PXAT" | "ex" | "px" | "exat" | "pxat")
                    if expiration_mode.is_normal() =>
                {
                    match values.next() {
                        Some(Value::BulkString(value)) => match Command::parse_integer(value) {
                            Ok(num) if num > 0 => {
                                expiration_mode = ExpirationMode::from(kind, num as u128)
                            }
                            Ok(_) => {
                                return Some(Command::build_error(
                                    "invalid expire time in 'set' command",
                                ))
                            }
                            Err(cmd) => return Some(cmd),
                        },
                        _ => return Some(Command::build_error("syntax error")),
                    }
                }
                "KEEPTTL" if expiration_mode.is_normal() => {
                    expiration_mode = ExpirationMode::KeepTTL
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
            "SET" | "set" => Command::parse_set_command(values),
            "EXISTS" | "exists" => Some(Self::Exists(values.collect())),
            string => Some(Self::Error(Value::Error(format!(
                "unknown command '{}'",
                string
            )))),
        };

        match cmd {
            Some(result) => Ok(result),
            None => Ok(Self::Error(Value::Error(format!(
                "wrong number of arguments for '{}' command",
                command.as_str()?
            )))),
        }
    }
}
