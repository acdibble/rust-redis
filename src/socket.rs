use crate::command::{Command, SetExpirationMode, SetInsertionMode};
use crate::store::Store;
use crate::value::Value;
use async_recursion::async_recursion;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Error, ErrorKind, Result},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

type Cache = Arc<Mutex<Store>>;

pub struct Socket {
    reader: BufReader<OwnedReadHalf>,
    write: OwnedWriteHalf,
    buffer: String,
    output_buffer: Vec<u8>,
    cache: Cache,
}

impl Socket {
    pub fn from(stream: TcpStream, cache: Cache) -> Self {
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
        // .map(|_| {
        //     println!("line: {:?}", self.buffer);
        // })
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

    async fn set(
        &mut self,
        key: Value,
        value: Value,
        insertion_mode: SetInsertionMode,
        expiration_mode: SetExpirationMode,
        return_mode: bool,
    ) -> Result<()> {
        let key = key.try_into()?;

        let mut store = self.cache.lock().expect("shouldn't fail to get a lock");

        let old_value = match store.insert(key, value, insertion_mode, expiration_mode) {
            Ok(None) if return_mode => Value::Nil,
            Ok(Some(value)) if return_mode => value,
            Ok(Some(_)) => Value::StaticSimpleString("OK"),
            Ok(None) => Value::StaticSimpleString("OK"),
            _ => Value::Nil,
        };

        write!(&mut self.output_buffer, "{}", old_value)
    }

    fn exists(&mut self, values: Vec<Value>) -> Result<()> {
        let store = self.cache.lock().expect("shouldn't fail to get a lock");
        let mut count = 0i64;

        for v in values {
            count += if store.has_entry(v.as_str()?) { 1 } else { 0 }
        }

        write!(&mut self.output_buffer, "{}", Value::Integer(count))
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            self.output_buffer.clear();

            let cmd = self.parse_command().await?;

            match cmd {
                Command::Echo(value) | Command::Ping(Some(value)) | Command::Error(value) => {
                    write!(&mut self.output_buffer, "{}", value)?
                }
                Command::Ping(None) => write!(
                    &mut self.output_buffer,
                    "{}",
                    Value::StaticSimpleString("PONG")
                )?,
                Command::Get(key) => self.fetch(key).await?,
                Command::Set {
                    key,
                    value,
                    insertion_mode,
                    expiration_mode,
                    return_mode,
                } => {
                    self.set(key, value, insertion_mode, expiration_mode, return_mode)
                        .await?
                }
                Command::Exists(values) => self.exists(values)?,
            }

            self.write.write_all(&self.output_buffer).await?
        }
    }
}
