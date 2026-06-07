use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub async fn read_lsp_stdio_message<R>(reader: &mut R) -> Result<Option<String>, String>
where
    R: AsyncRead + Unpin,
{
    let mut header = Vec::new();
    let mut byte = [0_u8; 1];

    loop {
        match reader.read_exact(&mut byte).await {
            Ok(_) => {
                header.push(byte[0]);
                if header.ends_with(b"\r\n\r\n") {
                    break;
                }
                if header.len() > 8192 {
                    return Err("language server response header is too large".to_owned());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                if header.is_empty() {
                    return Ok(None);
                }
                return Err("language server closed while sending response header".to_owned());
            }
            Err(error) => return Err(format!("failed to read language server header: {error}")),
        }
    }

    let header = String::from_utf8(header)
        .map_err(|_| "language server response header is not UTF-8".to_owned())?;
    let content_length = parse_content_length(&header)?;
    let mut body = vec![0_u8; content_length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|error| format!("failed to read language server body: {error}"))?;
    String::from_utf8(body)
        .map(Some)
        .map_err(|_| "language server response body is not UTF-8".to_owned())
}

pub async fn write_lsp_stdio_message<W>(writer: &mut W, body: &[u8]) -> Result<(), String>
where
    W: AsyncWrite + Unpin,
{
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await
        .map_err(|error| format!("failed to write language server header: {error}"))?;
    writer
        .write_all(body)
        .await
        .map_err(|error| format!("failed to write language server body: {error}"))?;
    writer
        .flush()
        .await
        .map_err(|error| format!("failed to flush language server message: {error}"))
}

fn parse_content_length(header: &str) -> Result<usize, String> {
    for line in header.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|_| "invalid language server Content-Length header".to_owned());
        }
    }

    Err("language server response missing Content-Length header".to_owned())
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;

    use super::{parse_content_length, read_lsp_stdio_message, write_lsp_stdio_message};

    #[test]
    fn parses_case_insensitive_content_length() {
        let length = parse_content_length(
            "content-length: 42\r\ncontent-type: application/vscode-jsonrpc\r\n\r\n",
        )
        .expect("content length");

        assert_eq!(length, 42);
    }

    #[tokio::test]
    async fn reads_stdio_framed_lsp_message() {
        let mut framed = std::io::Cursor::new(
            b"Content-Length: 24\r\nContent-Type: application/vscode-jsonrpc\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1}".to_vec(),
        );

        let message = read_lsp_stdio_message(&mut framed).await.expect("read").expect("message");

        assert_eq!(message, "{\"jsonrpc\":\"2.0\",\"id\":1}");
    }

    #[tokio::test]
    async fn writes_stdio_framed_lsp_message() {
        let (mut writer, mut reader) = tokio::io::duplex(128);
        let write = async move {
            write_lsp_stdio_message(&mut writer, br#"{"jsonrpc":"2.0"}"#).await.expect("write");
            writer.shutdown().await.expect("shutdown");
        };
        let read = async move {
            read_lsp_stdio_message(&mut reader).await.expect("read").expect("message")
        };

        let (_, message) = tokio::join!(write, read);

        assert_eq!(message, r#"{"jsonrpc":"2.0"}"#);
    }
}
