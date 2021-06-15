use futures_io::{AsyncRead, AsyncWrite};
use futures_lite::{AsyncReadExt, AsyncWriteExt as _};
use prost::Message;
use std::{convert::TryInto, io};

include!(concat!(env!("OUT_DIR"), "/zed.messages.rs"));

/// A message that the client can send to the server.
pub trait ClientMessage: Sized {
    fn to_variant(self) -> from_client::Variant;
    fn from_variant(variant: from_client::Variant) -> Option<Self>;
}

/// A message that the server can send to the client.
pub trait ServerMessage: Sized {
    fn to_variant(self) -> from_server::Variant;
    fn from_variant(variant: from_server::Variant) -> Option<Self>;
}

/// A message that the client can send to the server, where the server must respond with a single
/// message of a certain type.
pub trait RequestMessage: ClientMessage {
    type Response: ServerMessage;
}

/// A message that the client can send to the server, where the server must respond with a series of
/// messages of a certain type.
pub trait SubscribeMessage: ClientMessage {
    type Event: ServerMessage;
}

/// A message that the client can send to the server, where the server will not respond.
pub trait SendMessage: ClientMessage {}

macro_rules! directed_message {
    ($name:ident, $direction_trait:ident, $direction_module:ident) => {
        impl $direction_trait for $direction_module::$name {
            fn to_variant(self) -> $direction_module::Variant {
                $direction_module::Variant::$name(self)
            }

            fn from_variant(variant: $direction_module::Variant) -> Option<Self> {
                if let $direction_module::Variant::$name(msg) = variant {
                    Some(msg)
                } else {
                    None
                }
            }
        }
    };
}

macro_rules! request_message {
    ($req:ident, $resp:ident) => {
        directed_message!($req, ClientMessage, from_client);
        directed_message!($resp, ServerMessage, from_server);
        impl RequestMessage for from_client::$req {
            type Response = from_server::$resp;
        }
    };
}

macro_rules! send_message {
    ($msg:ident) => {
        directed_message!($msg, ClientMessage, from_client);
        impl SendMessage for from_client::$msg {}
    };
}

macro_rules! subscribe_message {
    ($subscription:ident, $event:ident) => {
        directed_message!($subscription, ClientMessage, from_client);
        directed_message!($event, ServerMessage, from_server);
        impl SubscribeMessage for from_client::$subscription {
            type Event = from_server::$event;
        }
    };
}

request_message!(Auth, AuthResponse);
request_message!(NewWorktree, NewWorktreeResponse);
request_message!(ShareWorktree, ShareWorktreeResponse);
send_message!(UploadFile);
subscribe_message!(SubscribeToPathRequests, PathRequest);

/// A stream of protobuf messages.
pub struct MessageStream<T> {
    byte_stream: T,
    buffer: Vec<u8>,
}

impl<T> MessageStream<T> {
    pub fn new(byte_stream: T) -> Self {
        Self {
            byte_stream,
            buffer: Default::default(),
        }
    }

    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.byte_stream
    }
}

impl<T> MessageStream<T>
where
    T: AsyncWrite + Unpin,
{
    /// Write a given protobuf message to the stream.
    pub async fn write_message(&mut self, message: &impl Message) -> io::Result<()> {
        let message_len: u32 = message
            .encoded_len()
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "message is too large"))?;
        self.buffer.clear();
        self.buffer.extend_from_slice(&message_len.to_be_bytes());
        message.encode(&mut self.buffer)?;
        self.byte_stream.write_all(&self.buffer).await
    }
}

impl<T> MessageStream<T>
where
    T: AsyncRead + Unpin,
{
    /// Read a protobuf message of the given type from the stream.
    pub async fn read_message<M: Message + Default>(&mut self) -> futures_io::Result<M> {
        let mut delimiter_buf = [0; 4];
        self.byte_stream.read_exact(&mut delimiter_buf).await?;
        let message_len = u32::from_be_bytes(delimiter_buf) as usize;
        self.buffer.resize(message_len, 0);
        self.byte_stream.read_exact(&mut self.buffer).await?;
        Ok(M::decode(self.buffer.as_slice())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };

    #[test]
    fn test_round_trip_message() {
        smol::block_on(async {
            let byte_stream = ChunkedStream {
                bytes: Vec::new(),
                read_offset: 0,
                chunk_size: 3,
            };

            let message1 = FromClient {
                id: 3,
                variant: Some(from_client::Variant::Auth(from_client::Auth {
                    user_id: 5,
                    access_token: "the-access-token".into(),
                })),
            };
            let message2 = FromClient {
                id: 4,
                variant: Some(from_client::Variant::UploadFile(from_client::UploadFile {
                    path: Vec::new(),
                    content: format!(
                        "a {}long error message that requires a two-byte length delimiter",
                        "very ".repeat(60)
                    )
                    .into(),
                })),
            };

            let mut message_stream = MessageStream::new(byte_stream);
            message_stream.write_message(&message1).await.unwrap();
            message_stream.write_message(&message2).await.unwrap();
            let decoded_message1 = message_stream.read_message::<FromClient>().await.unwrap();
            let decoded_message2 = message_stream.read_message::<FromClient>().await.unwrap();
            assert_eq!(decoded_message1, message1);
            assert_eq!(decoded_message2, message2);
        });
    }

    struct ChunkedStream {
        bytes: Vec<u8>,
        read_offset: usize,
        chunk_size: usize,
    }

    impl AsyncWrite for ChunkedStream {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let bytes_written = buf.len().min(self.chunk_size);
            self.bytes.extend_from_slice(&buf[0..bytes_written]);
            Poll::Ready(Ok(bytes_written))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncRead for ChunkedStream {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            let bytes_read = buf
                .len()
                .min(self.chunk_size)
                .min(self.bytes.len() - self.read_offset);
            let end_offset = self.read_offset + bytes_read;
            buf[0..bytes_read].copy_from_slice(&self.bytes[self.read_offset..end_offset]);
            self.read_offset = end_offset;
            Poll::Ready(Ok(bytes_read))
        }
    }
}
