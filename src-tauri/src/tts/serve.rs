//! Resident Pocket TTS server.
//!
//! The standalone `generate` sub-command reloads the whole model for every
//! phrase, which costs several seconds of silence before the first sound.
//! The pinned runtime also ships a `serve` sub-command that keeps the model
//! warm and streams the generated WAV, so Murmure starts it once over
//! loopback and posts each phrase to it.
//!
//! The server is bound to one language and one voice, the ones the user
//! selected: the runtime loads the voice when it starts, so a change of
//! language or of voice replaces the server instead of being ignored.
//!
//! The server is loopback only: it is bound to 127.0.0.1 on a free port, it
//! never receives the document text through process arguments, and the model
//! cache stays the dedicated one, with no inherited Hugging Face credentials.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

/// The first start may have to download the model the user approved, so the
/// readiness bound matches the generation bound instead of a short timeout.
const READY_TIMEOUT: Duration = Duration::from_secs(600);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const READY_POLL: Duration = Duration::from_millis(100);

struct Server {
    child: Child,
    port: u16,
    /// Language the server was started with.
    language: String,
    /// Voice the server was started with.
    voice: String,
    /// Windows job handle, stored as an integer because a raw pointer is not
    /// `Send`. Closing it kills the server even when the application crashes.
    job: isize,
}

impl Drop for Server {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if self.job != 0 {
                unsafe {
                    windows_sys::Win32::Foundation::CloseHandle(self.job as _);
                }
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

static SERVER: Mutex<Option<Server>> = Mutex::new(None);

/// Return the port of a warm server for `language` and `voice`, starting it
/// when needed.
///
/// A server that exited since the last request is replaced, so a runtime
/// crash never leaves the feature unusable until the next restart. A change
/// of language or of voice also replaces it, because the runtime loads both
/// when it starts.
pub(super) fn ensure_running(
    binary: &Path,
    language: &str,
    voice: &str,
    cache_dir: &Path,
) -> Result<u16, String> {
    let mut guard = SERVER
        .lock()
        .map_err(|_| "The Pocket TTS server state is unavailable".to_string())?;
    if let Some(server) = guard.as_mut() {
        if server.language == language
            && server.voice == voice
            && matches!(server.child.try_wait(), Ok(None))
        {
            return Ok(server.port);
        }
    }
    *guard = None;
    let server = Server::start(binary, language, voice, cache_dir)?;
    let port = server.port;
    *guard = Some(server);
    Ok(port)
}

/// Stop the server, if one is running.
pub(super) fn shutdown() {
    if let Ok(mut guard) = SERVER.lock() {
        *guard = None;
    }
}

/// Drop the server after a generation was abandoned mid-stream.
///
/// The runtime handles one phrase at a time: a generation the user stopped
/// keeps running inside the server and delays the next request, which then
/// starves the playback. Killing the process is the only cancellation the
/// local API exposes; the next request starts a fresh one.
pub(super) fn abandon_generation() {
    thread::spawn(shutdown);
}

impl Server {
    fn start(binary: &Path, language: &str, voice: &str, cache_dir: &Path) -> Result<Self, String> {
        let port = free_loopback_port()?;
        let mut command = Command::new(binary);
        super::hide_console_window(&mut command);
        super::preview::configure_model_source(&mut command, cache_dir);
        command
            .args(server_args(port, language, voice))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command
            .spawn()
            .map_err(|_| "Unable to start the Pocket TTS server".to_string())?;
        raise_generation_priority(&child);
        let job = attach_kill_on_close_job(&child);
        let mut server = Self {
            child,
            port,
            language: language.to_string(),
            voice: voice.to_string(),
            job,
        };
        server.wait_until_ready()?;
        Ok(server)
    }

    /// The runtime loads the model before it starts listening, so accepting a
    /// connection proves that the next request does not pay that cost again.
    fn wait_until_ready(&mut self) -> Result<(), String> {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, self.port));
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            if TcpStream::connect_timeout(&address, READY_POLL).is_ok() {
                return Ok(());
            }
            if !matches!(self.child.try_wait(), Ok(None)) {
                return Err("The Pocket TTS server stopped before it was ready".to_string());
            }
            if Instant::now() >= deadline {
                return Err("The Pocket TTS server did not start in time".to_string());
            }
            thread::sleep(READY_POLL);
        }
    }
}

/// The arguments that start the resident server.
///
/// The runtime loads the voice when it starts, so the selected voice has to
/// be part of the command line, and never of the request body.
fn server_args(port: u16, language: &str, voice: &str) -> Vec<std::ffi::OsString> {
    use std::ffi::OsString;
    vec![
        OsString::from("serve"),
        OsString::from("--host"),
        OsString::from("127.0.0.1"),
        OsString::from("--port"),
        OsString::from(port.to_string()),
        OsString::from("--language"),
        OsString::from(language),
        OsString::from("--default-voice"),
        OsString::from(voice),
    ]
}

/// Ask the runtime to bind an ephemeral port so a second instance of the
/// application, or an unrelated service, cannot collide with it.
fn free_loopback_port() -> Result<u16, String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|_| "No loopback port is available for Pocket TTS".to_string())?;
    let port = listener
        .local_addr()
        .map_err(|_| "No loopback port is available for Pocket TTS".to_string())?
        .port();
    drop(listener);
    Ok(port)
}

/// Synthesis must keep up with playback, which is a real time constraint the
/// user's background work competes with. Above normal (never high) let the
/// generation hold its rate under load without starving the foreground: on a
/// loaded machine the measured factor went from 0.46 to 0.28 of real time.
#[cfg(target_os = "windows")]
fn raise_generation_priority(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Threading::{SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS};

    // Best effort: a refused priority only means the default one applies.
    unsafe {
        let _ = SetPriorityClass(child.as_raw_handle(), ABOVE_NORMAL_PRIORITY_CLASS);
    }
}

#[cfg(not(target_os = "windows"))]
fn raise_generation_priority(_child: &Child) {}

/// Bind the server to a job that kills it when the handle is closed, which is
/// also what happens if the application crashes. Best effort: a host that
/// already runs in a restricted job may refuse the assignment, and the
/// explicit kill on shutdown still applies.
#[cfg(target_os = "windows")]
fn attach_kill_on_close_job(child: &Child) -> isize {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return 0;
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        let assigned = configured != 0 && AssignProcessToJobObject(job, child.as_raw_handle()) != 0;
        if !assigned {
            windows_sys::Win32::Foundation::CloseHandle(job);
            return 0;
        }
        job as isize
    }
}

#[cfg(not(target_os = "windows"))]
fn attach_kill_on_close_job(_child: &Child) -> isize {
    0
}

/// Post one phrase and return the streamed WAV body.
///
/// The text travels in the request body, never in a process argument, and the
/// response is consumed as it arrives so playback can start before the whole
/// sentence is generated.
pub(super) fn request_speech(port: u16, text: &str) -> Result<Box<dyn Read + Send>, String> {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)
        .map_err(|_| "The Pocket TTS server is not reachable".to_string())?;
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(|_| "The Pocket TTS server could not be configured".to_string())?;
    stream
        .set_write_timeout(Some(READ_TIMEOUT))
        .map_err(|_| "The Pocket TTS server could not be configured".to_string())?;
    stream.set_nodelay(true).ok();

    let boundary = format!("murmure-{}", uuid::Uuid::new_v4().simple());
    let body = multipart_body(&boundary, text);
    let head = request_head(&boundary, body.len());

    let mut writer = stream
        .try_clone()
        .map_err(|_| "The Pocket TTS server is not reachable".to_string())?;
    writer
        .write_all(head.as_bytes())
        .and_then(|()| writer.write_all(&body))
        .map_err(|_| "Unable to send the text to Pocket TTS".to_string())?;
    writer.flush().ok();
    drop(writer);

    let mut reader = BufReader::new(stream);
    let response = read_response_head(&mut reader)?;
    if response.status != 200 {
        return Err(format!(
            "Pocket TTS answered with status {}",
            response.status
        ));
    }
    if response.chunked {
        Ok(Box::new(Chunked::new(reader)))
    } else {
        Ok(Box::new(Limited {
            inner: reader,
            remaining: response.length.unwrap_or(u64::MAX),
        }))
    }
}

fn request_head(boundary: &str, body_len: usize) -> String {
    format!(
        "POST /tts HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: multipart/form-data; boundary={boundary}\r\nContent-Length: {body_len}\r\nConnection: close\r\n\r\n"
    )
}

fn multipart_body(boundary: &str, text: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(text.len() + boundary.len() * 2 + 96);
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"text\"\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(text.as_bytes());
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

struct ResponseHead {
    status: u16,
    chunked: bool,
    length: Option<u64>,
}

fn read_response_head<R: BufRead>(reader: &mut R) -> Result<ResponseHead, String> {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|_| "The Pocket TTS server did not answer".to_string())?;
    let status = line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| "The Pocket TTS server answered without a status".to_string())?;
    let mut chunked = false;
    let mut length = None;
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|_| "The Pocket TTS server did not answer".to_string())?;
        if read == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim();
            match name.trim().to_ascii_lowercase().as_str() {
                "transfer-encoding" if value.eq_ignore_ascii_case("chunked") => chunked = true,
                "content-length" => length = value.parse::<u64>().ok(),
                _ => {}
            }
        }
    }
    Ok(ResponseHead {
        status,
        chunked,
        length,
    })
}

/// Decode the `Transfer-Encoding: chunked` framing the runtime uses when it
/// streams a sentence.
struct Chunked<R: BufRead> {
    inner: R,
    remaining: usize,
    finished: bool,
}

impl<R: BufRead> Chunked<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            remaining: 0,
            finished: false,
        }
    }

    fn read_chunk_size(&mut self) -> std::io::Result<usize> {
        let mut line = String::new();
        let read = self.inner.read_line(&mut line)?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the chunked body ended before its last chunk",
            ));
        }
        let size = line.trim().split(';').next().unwrap_or_default().trim();
        usize::from_str_radix(size, 16).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "the chunked body carries an invalid chunk size",
            )
        })
    }

    fn consume_separator(&mut self) -> std::io::Result<()> {
        let mut separator = [0_u8; 2];
        self.inner.read_exact(&mut separator)
    }
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.finished || buf.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            self.remaining = self.read_chunk_size()?;
            if self.remaining == 0 {
                self.finished = true;
                return Ok(0);
            }
        }
        let wanted = buf.len().min(self.remaining);
        let read = self.inner.read(&mut buf[..wanted])?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "the chunked body ended inside a chunk",
            ));
        }
        self.remaining -= read;
        if self.remaining == 0 {
            self.consume_separator()?;
        }
        Ok(read)
    }
}

/// Stop at the announced length so a keep-alive connection cannot be mistaken
/// for a longer body.
struct Limited<R: Read> {
    inner: R,
    remaining: u64,
}

impl<R: Read> Read for Limited<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 || buf.is_empty() {
            return Ok(0);
        }
        let wanted = usize::try_from(self.remaining)
            .map(|left| left.min(buf.len()))
            .unwrap_or(buf.len());
        let read = self.inner.read(&mut buf[..wanted])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::TcpListener;

    /// The resident server loads the voice when it starts, so the selected
    /// language and the selected voice must both reach the runtime.
    #[test]
    fn the_server_is_started_with_the_selected_language_and_voice() {
        let parts: Vec<String> = server_args(4242, "spanish", "lola")
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(parts[0], "serve");
        assert!(parts.windows(2).any(|p| p == ["--language", "spanish"]));
        assert!(parts.windows(2).any(|p| p == ["--default-voice", "lola"]));
        assert!(parts.windows(2).any(|p| p == ["--host", "127.0.0.1"]));
        assert!(parts.windows(2).any(|p| p == ["--port", "4242"]));
    }

    #[test]
    fn the_request_declares_the_boundary_and_the_exact_body_length() {
        let body = multipart_body("abc", "bonjour");
        let head = request_head("abc", body.len());
        assert!(head.starts_with("POST /tts HTTP/1.1\r\n"));
        assert!(head.contains("Content-Type: multipart/form-data; boundary=abc\r\n"));
        assert!(head.contains(&format!("Content-Length: {}\r\n", body.len())));
        assert!(head.ends_with("\r\n\r\n"));
        assert!(String::from_utf8_lossy(&body).contains("name=\"text\"\r\n\r\nbonjour\r\n--abc--"));
    }

    #[test]
    fn a_chunked_body_is_decoded_across_chunk_boundaries() {
        let mut wire = Cursor::new(b"4\r\nRIFF\r\n5\r\nWAVE!\r\n0\r\n\r\n");
        let mut chunked = Chunked {
            inner: &mut wire,
            remaining: 0,
            finished: false,
        };
        let mut decoded = Vec::new();
        chunked.read_to_end(&mut decoded).unwrap();
        assert_eq!(decoded, b"RIFFWAVE!");
    }

    #[test]
    fn a_chunked_body_with_a_broken_size_is_an_error() {
        let mut wire = Cursor::new(b"zz\r\nRIFF\r\n");
        let mut chunked = Chunked {
            inner: &mut wire,
            remaining: 0,
            finished: false,
        };
        let mut decoded = Vec::new();
        assert!(chunked.read_to_end(&mut decoded).is_err());
    }

    #[test]
    fn a_body_announced_by_length_stops_at_that_length() {
        let wire = Cursor::new(b"RIFFextra");
        let mut limited = Limited {
            inner: wire,
            remaining: 4,
        };
        let mut decoded = Vec::new();
        limited.read_to_end(&mut decoded).unwrap();
        assert_eq!(decoded, b"RIFF");
    }

    #[test]
    fn the_response_head_reports_status_and_framing() {
        let mut wire = Cursor::new(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: audio/wav\r\n\r\n",
        );
        let head = read_response_head(&mut wire).unwrap();
        assert_eq!(head.status, 200);
        assert!(head.chunked);

        let mut wire = Cursor::new(b"HTTP/1.1 500 Oops\r\nContent-Length: 0\r\n\r\n");
        let head = read_response_head(&mut wire).unwrap();
        assert_eq!(head.status, 500);
        assert_eq!(head.length, Some(0));
    }

    #[test]
    fn a_response_without_a_status_is_refused() {
        let mut wire = Cursor::new(b"garbage\r\n\r\n");
        assert!(read_response_head(&mut wire).is_err());
    }

    #[test]
    fn the_free_port_can_be_bound_again() {
        let port = free_loopback_port().unwrap();
        assert_ne!(port, 0);
        let rebound = TcpListener::bind((Ipv4Addr::LOCALHOST, port));
        assert!(rebound.is_ok(), "the announced port must be usable");
    }

    #[test]
    fn a_local_server_receives_the_text_and_returns_the_streamed_body() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            // Read the head, then exactly the announced body.
            loop {
                let read = socket.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&request[..end]).to_string();
                    let length = head
                        .lines()
                        .find_map(|line| line.strip_prefix("Content-Length: "))
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let text = String::from_utf8_lossy(&request);
            assert!(text.contains("bonjour la synthese"));
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nRIFF\r\n0\r\n\r\n",
                )
                .unwrap();
            socket.flush().unwrap();
        });

        let mut body = request_speech(port, "bonjour la synthese").unwrap();
        let mut received = Vec::new();
        body.read_to_end(&mut received).unwrap();
        assert_eq!(received, b"RIFF");
        server.join().unwrap();
    }
}
