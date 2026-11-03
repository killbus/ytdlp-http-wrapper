use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_VERSION: u32 = 1;
// A result may retain 10 MiB from each output stream. JSON can encode each
// control byte as a six-byte escape, so the frame budget must cover the
// worst-case representation plus envelope overhead.
pub const MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Hello,
    Ready,
    Run,
    Result,
    Error,
    Cancel,
    Shutdown,
    Ping,
    Pong,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub protocol_version: u32,
    pub message_type: MessageType,
    pub request_id: String,
    pub payload: Value,
}

impl Frame {
    pub fn new(
        message_type: MessageType,
        request_id: impl Into<String>,
        payload: Value,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            protocol_version: PROTOCOL_VERSION,
            message_type,
            request_id: request_id.into(),
            payload,
        })
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        if self.request_id.is_empty() {
            return Err(ProtocolError::EmptyRequestId);
        }
        Ok(())
    }

    pub fn validate_request_id(&self, expected: &str) -> Result<(), ProtocolError> {
        if self.request_id != expected {
            return Err(ProtocolError::RequestIdMismatch {
                expected: expected.to_owned(),
                actual: self.request_id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    Json(serde_json::Error),
    FrameTooLarge { length: usize, limit: usize },
    TruncatedPrefix,
    TruncatedPayload { expected: usize, received: usize },
    EmptyRequestId,
    RequestIdMismatch { expected: String, actual: String },
    VersionMismatch { expected: u32, actual: u32 },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "worker protocol I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "worker protocol JSON failed: {error}"),
            Self::FrameTooLarge { length, limit } => {
                write!(
                    formatter,
                    "worker frame length {length} exceeds limit {limit}"
                )
            }
            Self::TruncatedPrefix => write!(formatter, "worker frame prefix was truncated"),
            Self::TruncatedPayload { expected, received } => write!(
                formatter,
                "worker frame payload was truncated: expected {expected} bytes, got {received}"
            ),
            Self::EmptyRequestId => write!(formatter, "worker frame request_id is empty"),
            Self::RequestIdMismatch { expected, actual } => write!(
                formatter,
                "worker frame request_id mismatch: expected {expected}, got {actual}"
            ),
            Self::VersionMismatch { expected, actual } => write!(
                formatter,
                "worker protocol version mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

pub async fn write_frame<W>(writer: &mut W, frame: &Frame) -> Result<(), ProtocolError>
where
    W: AsyncWrite + Unpin,
{
    frame.validate()?;
    let encoded = serde_json::to_vec(frame).map_err(ProtocolError::Json)?;
    if encoded.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge {
            length: encoded.len(),
            limit: MAX_FRAME_BYTES,
        });
    }
    let length = u32::try_from(encoded.len()).map_err(|_| ProtocolError::FrameTooLarge {
        length: encoded.len(),
        limit: MAX_FRAME_BYTES,
    })?;
    writer
        .write_all(&length.to_be_bytes())
        .await
        .map_err(ProtocolError::Io)?;
    writer
        .write_all(&encoded)
        .await
        .map_err(ProtocolError::Io)?;
    writer.flush().await.map_err(ProtocolError::Io)
}

pub async fn read_frame<R>(reader: &mut R) -> Result<Option<Frame>, ProtocolError>
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0_u8; 4];
    let first = reader
        .read(&mut prefix[..1])
        .await
        .map_err(ProtocolError::Io)?;
    if first == 0 {
        return Ok(None);
    }
    if let Err(error) = reader.read_exact(&mut prefix[1..]).await {
        return if error.kind() == io::ErrorKind::UnexpectedEof {
            Err(ProtocolError::TruncatedPrefix)
        } else {
            Err(ProtocolError::Io(error))
        };
    }

    let length = u32::from_be_bytes(prefix) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge {
            length,
            limit: MAX_FRAME_BYTES,
        });
    }

    let mut encoded = vec![0_u8; length];
    let mut received = 0;
    while received < length {
        match reader.read(&mut encoded[received..]).await {
            Ok(0) => {
                return Err(ProtocolError::TruncatedPayload {
                    expected: length,
                    received,
                });
            }
            Ok(size) => received += size,
            Err(error) => return Err(ProtocolError::Io(error)),
        }
    }
    let frame: Frame = serde_json::from_slice(&encoded).map_err(ProtocolError::Json)?;
    frame.validate()?;
    Ok(Some(frame))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{duplex, AsyncWriteExt};

    #[tokio::test]
    async fn frame_round_trip_survives_partial_transport_reads() {
        let frame = Frame::new(
            MessageType::Run,
            "request-42",
            json!({"args": ["--version"]}),
        )
        .unwrap();
        let (mut client, mut server) = duplex(8);
        let expected = frame.clone();

        let writer = tokio::spawn(async move { write_frame(&mut client, &frame).await });
        let decoded = read_frame(&mut server).await.unwrap().unwrap();

        writer.await.unwrap().unwrap();
        assert_eq!(decoded, expected);
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected_before_allocation() {
        let (mut client, mut server) = duplex(8);
        client
            .write_all(&((MAX_FRAME_BYTES as u32) + 1).to_be_bytes())
            .await
            .unwrap();

        let error = read_frame(&mut server).await.unwrap_err();
        assert!(matches!(
            error,
            ProtocolError::FrameTooLarge {
                length,
                limit: MAX_FRAME_BYTES
            } if length == MAX_FRAME_BYTES + 1
        ));
    }

    #[tokio::test]
    async fn truncated_prefix_and_payload_are_not_clean_eof() {
        let (mut prefix_writer, mut prefix_reader) = duplex(8);
        prefix_writer.write_all(&[0, 0]).await.unwrap();
        prefix_writer.shutdown().await.unwrap();
        assert!(matches!(
            read_frame(&mut prefix_reader).await.unwrap_err(),
            ProtocolError::TruncatedPrefix
        ));

        let (mut payload_writer, mut payload_reader) = duplex(16);
        payload_writer
            .write_all(&12_u32.to_be_bytes())
            .await
            .unwrap();
        payload_writer.write_all(b"short").await.unwrap();
        payload_writer.shutdown().await.unwrap();
        assert!(matches!(
            read_frame(&mut payload_reader).await.unwrap_err(),
            ProtocolError::TruncatedPayload {
                expected: 12,
                received: 5
            }
        ));
    }

    #[tokio::test]
    async fn malformed_and_unknown_frames_are_rejected() {
        let malformed = b"not-json";
        let (mut malformed_writer, mut malformed_reader) = duplex(64);
        malformed_writer
            .write_all(&(malformed.len() as u32).to_be_bytes())
            .await
            .unwrap();
        malformed_writer.write_all(malformed).await.unwrap();
        assert!(matches!(
            read_frame(&mut malformed_reader).await.unwrap_err(),
            ProtocolError::Json(_)
        ));

        let unknown_field = json!({
            "protocol_version": PROTOCOL_VERSION,
            "message_type": "result",
            "request_id": "request-1",
            "payload": {"exit_code": 0},
            "unexpected": true
        });
        let encoded = serde_json::to_vec(&unknown_field).unwrap();
        let (mut writer, mut reader) = duplex(256);
        writer
            .write_all(&(encoded.len() as u32).to_be_bytes())
            .await
            .unwrap();
        writer.write_all(&encoded).await.unwrap();
        assert!(matches!(
            read_frame(&mut reader).await.unwrap_err(),
            ProtocolError::Json(_)
        ));
    }

    #[tokio::test]
    async fn mismatched_version_and_request_id_are_rejected() {
        let payload = json!({});
        let wrong_version = Frame {
            protocol_version: PROTOCOL_VERSION + 1,
            message_type: MessageType::Hello,
            request_id: "startup-1".into(),
            payload: payload.clone(),
        };
        assert!(matches!(
            wrong_version.validate().unwrap_err(),
            ProtocolError::VersionMismatch { .. }
        ));

        let empty_request = Frame {
            protocol_version: PROTOCOL_VERSION,
            message_type: MessageType::Hello,
            request_id: String::new(),
            payload,
        };
        assert!(matches!(
            empty_request.validate().unwrap_err(),
            ProtocolError::EmptyRequestId
        ));

        let frame = Frame::new(MessageType::Result, "request-2", json!({})).unwrap();
        assert!(matches!(
            frame.validate_request_id("request-1").unwrap_err(),
            ProtocolError::RequestIdMismatch { expected, actual }
                if expected == "request-1" && actual == "request-2"
        ));
        frame.validate_request_id("request-2").unwrap();
    }
}
