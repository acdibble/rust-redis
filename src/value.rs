use tokio::io::{Error, ErrorKind, Result};

#[derive(Debug)]
pub enum Value {
    Nil,
    SimpleString(String),
    StaticSimpleString(&'static str),
    BulkString(String),
    Array(Vec<Value>),
    StaticError(&'static str),
    Error(String),
}

impl Value {
    pub fn into_array(self) -> Result<Vec<Value>> {
        match self {
            Value::Array(values) => Ok(values),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("expected array, got {:?}", self),
            )),
        }
    }

    pub fn as_str(&self) -> Result<&str> {
        match self {
            Self::BulkString(value) | Self::SimpleString(value) => Ok(value.as_str()),
            Self::StaticSimpleString(value) => Ok(value),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("expected array, got {:?}", self),
            )),
        }
    }
}

impl TryInto<String> for Value {
    type Error = Error;

    fn try_into(self) -> std::result::Result<String, <Self as TryInto<String>>::Error> {
        match self {
            Self::BulkString(value) | Self::SimpleString(value) => Ok(value),
            Self::StaticSimpleString(value) => Ok(value.to_owned()),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("expected string, got {:?}", self),
            )),
        }
    }
}

impl TryInto<u64> for Value {
    type Error = Error;

    fn try_into(self) -> std::result::Result<u64, <Self as TryInto<u64>>::Error> {
        self.as_str()?.parse().or_else(|_| {
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!("failed to convert value to u64: {:?}", self),
            ))
        })
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nil => write!(f, "$-1\r\n"),
            Self::StaticSimpleString(value) => write!(f, "+{}\r\n", value),
            Self::SimpleString(value) => write!(f, "+{}\r\n", value),
            Self::BulkString(value) => write!(f, "${}\r\n{}\r\n", value.len(), value),
            Self::Array(values) => {
                write!(f, "*{}\r\n", values.len())?;

                for v in values {
                    write!(f, "{}\r\n", v)?;
                }

                Ok(())
            }
            Self::Error(message) => write!(f, "-{}\r\n", message),
            Self::StaticError(message) => write!(f, "-{}\r\n", message),
        }
    }
}
