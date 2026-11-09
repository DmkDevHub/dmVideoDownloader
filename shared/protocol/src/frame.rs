use crate::{Code, Message, MAX_BYTES};
use std::io::{self, Read, Write};

pub fn read(input: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; 4];
    loop {
        match input.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    input.read_exact(&mut header[1..])?;
    let len = u32::from_ne_bytes(header) as usize;
    if !(1..=MAX_BYTES).contains(&len) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame length",
        ));
    }
    let mut bytes = vec![0; len];
    input.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

pub fn write(output: &mut impl Write, message: &Message) -> io::Result<()> {
    let bytes = message
        .encode()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid message"))?;
    output.write_all(&(bytes.len() as u32).to_ne_bytes())?;
    output.write_all(&bytes)?;
    output.flush()
}

pub fn exchange(
    input: &mut impl Read,
    output: &mut impl Write,
    mut handle: impl FnMut(Message) -> Message,
) -> io::Result<()> {
    while let Some(bytes) = read(input)? {
        let reply = match Message::decode(&bytes) {
            Ok(message) => handle(message),
            Err(code) => Message::error(
                crate::request_id_hint(&bytes).unwrap_or_else(|| "invalid".into()),
                code,
            ),
        };
        write(output, &reply)?;
    }
    Ok(())
}

pub fn request_only(message: &Message) -> Result<(), Code> {
    match message.body {
        crate::Body::VideoCandidate(_)
        | crate::Body::Ping(_)
        | crate::Body::ConnectionTestResult(_)
        | crate::Body::GetRuntimeStatus(_) => Ok(()),
        _ => Err(Code::InvalidPayload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct OneByte<'a>(&'a [u8]);
    impl Read for OneByte<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(&mut buf[..1])
        }
    }
    #[test]
    fn split_and_consecutive_frames() {
        let mut bytes = Vec::new();
        for id in ["first", "second"] {
            write(&mut bytes, &Message::error(id, Code::AppNotRunning)).unwrap();
        }
        let mut input = OneByte(&bytes);
        for id in ["first", "second"] {
            let decoded = Message::decode(&read(&mut input).unwrap().unwrap()).unwrap();
            assert_eq!(decoded.request_id, id);
        }
        assert!(read(&mut input).unwrap().is_none());
    }
    #[test]
    fn invalid_and_truncated_lengths() {
        for len in [0, MAX_BYTES as u32 + 1] {
            assert_eq!(
                read(&mut len.to_ne_bytes().as_slice()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        for bytes in [&[1, 0][..], &[3, 0, 0, 0, 1][..]] {
            assert_eq!(
                read(&mut &*bytes).unwrap_err().kind(),
                io::ErrorKind::UnexpectedEof
            );
        }
    }
}
