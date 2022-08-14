mod store;

use crate::store::Store;
use async_recursion::async_recursion;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Error, ErrorKind, Result},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
};

type Cache = Arc<Mutex<Store<Value>>>;

#[derive(Debug)]
enum Command {
    Ping(Option<Value>),
    Echo(Value),
    Get(Value),
    Set(Value, Value),
    DynamicError(Value),
    StaticError(Value),
}

impl TryFrom<Value> for Command {
    type Error = Error;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        let mut values = value.into_array()?;
        let command = values[0].as_str()?;

        let cmd = match command {
            "PING" | "ping" => {
                if values.len() == 1 {
                    Some(Self::Ping(None))
                } else if values.len() == 2 {
                    Some(Self::Ping(values.pop()))
                } else {
                    None
                }
            }
            "ECHO" | "echo" => {
                if values.len() == 2 {
                    Some(Self::Echo(values.pop().unwrap()))
                } else {
                    None
                }
            }
            "GET" | "get" => {
                if values.len() == 2 {
                    Some(Self::Get(values.pop().unwrap()))
                } else {
                    None
                }
            }
            "SET" | "set" => {
                if values.len() == 3 {
                    let value = values.pop().unwrap();
                    Some(Self::Set(values.pop().unwrap(), value))
                } else {
                    None
                }
            }
            string => Some(Self::DynamicError(Value::Error(format!(
                "unknown command '{}'",
                string
            )))),
        };

        match cmd {
            Some(result) => Ok(result),
            None => Ok(Self::StaticError(Value::StaticError(
                "ERR wrong number of arguments for command",
            ))),
        }
    }
}

#[derive(Debug)]
enum Value {
    Nil,
    SimpleString(String),
    StaticSimpleString(&'static str),
    BulkString(String),
    Array(Vec<Value>),
    StaticError(&'static str),
    Error(String),
}

impl Value {
    fn into_array(self) -> Result<Vec<Value>> {
        match self {
            Value::Array(values) => Ok(values),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("expected array, got {:?}", self),
            )),
        }
    }

    fn as_str(&self) -> Result<&str> {
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

struct Socket {
    reader: BufReader<OwnedReadHalf>,
    write: OwnedWriteHalf,
    buffer: String,
    output_buffer: Vec<u8>,
    cache: Cache,
}

impl Socket {
    fn from(stream: TcpStream, cache: Cache) -> Self {
        let (read, write) = stream.into_split();

        Self {
            write,
            reader: BufReader::new(read),
            buffer: String::with_capacity(1024),
            output_buffer: Vec::with_capacity(1024),
            cache,
        }
    }

    async fn read_line(&mut self) -> Result<()> {
        self.buffer.clear();
        match self.reader.read_line(&mut self.buffer).await {
            Ok(0) => Err(Error::new(
                ErrorKind::ConnectionAborted,
                "socket connection was closed",
            )),
            Ok(_) => Ok(()),
            _ => Err(Error::new(
                ErrorKind::Other,
                "failed to read line from buffer",
            )),
        }
        .map(|_| {
            println!("line: {:?}", self.buffer);
        })
    }

    fn parse_len(&self) -> Result<i32> {
        let length_of_string = self.buffer.len();
        match self.buffer[1..length_of_string - 2].parse() {
            Ok(result) => Ok(result),
            Err(_) => Err(Error::new(
                ErrorKind::InvalidInput,
                "failed to parse length of value",
            )),
        }
    }

    async fn parse_bulk_string(&mut self) -> Result<Value> {
        let string_len = self.parse_len()?;

        if string_len == -1 {
            return Ok(Value::Nil);
        }

        if string_len < 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "bulk string cannot have negative length",
            ));
        }

        self.read_line().await?;

        if self.buffer.len() - 2 != string_len as usize {
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "bulk string length mismatch: {:?} {:?}",
                    string_len, self.buffer
                ),
            ))
        } else if string_len == 0 {
            Ok(Value::BulkString(String::new()))
        } else {
            Ok(Value::BulkString(String::from(
                &self.buffer[0..string_len as usize],
            )))
        }
    }

    async fn parse_array(&mut self) -> Result<Value> {
        let array_len = self.parse_len()?;

        if array_len == -1 {
            return Ok(Value::Nil);
        }

        if array_len < 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "array cannot have negative length",
            ));
        }

        let mut values = Vec::with_capacity(array_len as usize);

        for _ in 0..array_len {
            values.push(self.parse_value().await?);
        }

        Ok(Value::Array(values))
    }

    async fn parse_simple_string(&mut self) -> Result<Value> {
        Ok(Value::SimpleString(String::from(
            &self.buffer[1..self.buffer.len() - 2],
        )))
    }

    #[async_recursion]
    async fn parse_value(&mut self) -> Result<Value> {
        self.read_line().await?;

        match &self.buffer[0..1] {
            "*" => self.parse_array().await,
            "$" => self.parse_bulk_string().await,
            "+" => self.parse_simple_string().await,
            _ => todo!("handled {:?}", self.buffer),
        }
    }

    async fn parse_command(&mut self) -> Result<Command> {
        let array = self.parse_value().await?;

        Command::try_from(array)
    }

    async fn fetch(&mut self, key: Value) -> Result<()> {
        let store = self.cache.lock().expect("shouldn't fail to get a lock");

        let key = key.as_str()?;

        let value = match store.get(key) {
            Some(value) => value,
            _ => &Value::Nil,
        };

        write!(&mut self.output_buffer, "{}", value)
    }

    async fn set(&mut self, key: Value, value: Value) -> Result<()> {
        let key = key.try_into()?;

        let mut store = self.cache.lock().expect("shouldn't fail to get a lock");

        store.insert(key, value);

        write!(
            &mut self.output_buffer,
            "{}",
            Value::StaticSimpleString("OK")
        )
    }

    async fn run(&mut self) -> Result<()> {
        loop {
            let cmd = self.parse_command().await?;

            println!("command: {cmd:?}");

            match cmd {
                Command::Echo(value)
                | Command::Ping(Some(value))
                | Command::StaticError(value)
                | Command::DynamicError(value) => write!(&mut self.output_buffer, "{}", value)?,
                Command::Ping(None) => write!(
                    &mut self.output_buffer,
                    "{}",
                    Value::StaticSimpleString("PONG")
                )?,
                Command::Get(key) => self.fetch(key).await?,
                Command::Set(key, value) => self.set(key, value).await?,
            }

            self.write.write_all(&self.output_buffer).await?
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    let cache = Arc::new(Mutex::new(Store::<Value>::new()));

    loop {
        let (socket, addr) = listener.accept().await?;

        println!("accepted connection from {}", addr);

        let clone = cache.clone();

        tokio::spawn(async move {
            match Socket::from(socket, clone).run().await {
                Ok(()) => {}
                Err(error) => eprintln!("encountered error: {}", error),
            }

            println!("dropped connection with {}", addr);
        });
    }
}
