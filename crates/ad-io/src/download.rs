//! Map image download from the data repository (AD-9, AD-14). Runs on a job, never on the
//! UI thread.

use std::time::Duration;

/// Where map images are downloaded from: `<base>/<name>/<name>.png`.
pub const MAP_IMAGES_URL: &str =
    "https://raw.githubusercontent.com/KillBait/AutoDrive_MapImages/master/mapImages";

/// Timeouts and the size cap of a download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub connect: Duration,
    /// The whole request, from connect to the last body byte.
    pub global: Duration,
    pub max_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            global: Duration::from_secs(60),
            max_bytes: 32 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    #[error("not found")]
    NotFound,
    #[error("HTTP status {0}")]
    Status(u16),
    #[error("timed out")]
    Timeout,
    #[error("larger than {0} bytes")]
    TooLarge(u64),
    #[error("{0}")]
    Network(String),
}

/// `<base_url>/<name>/<name>.png`, the name percent-encoded.
pub fn map_image_url(base_url: &str, name: &str) -> String {
    let name: String = name
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("{}/{name}/{name}.png", base_url.trim_end_matches('/'))
}

/// GETs the map image of `name` below `base_url`. 404 is `NotFound`, any other status but
/// 200 is `Status`; a body above `limits.max_bytes` is `TooLarge`, never truncated.
pub fn fetch_map_image(
    base_url: &str,
    name: &str,
    limits: Limits,
) -> Result<Vec<u8>, DownloadError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(limits.connect))
        .timeout_global(Some(limits.global))
        .http_status_as_error(false)
        .build()
        .into();
    let fail = |e: ureq::Error| match e {
        ureq::Error::Timeout(_) => DownloadError::Timeout,
        ureq::Error::Io(e) if e.kind() == std::io::ErrorKind::TimedOut => DownloadError::Timeout,
        ureq::Error::BodyExceedsLimit(_) => DownloadError::TooLarge(limits.max_bytes),
        other => DownloadError::Network(other.to_string()),
    };
    let mut response = agent
        .get(&map_image_url(base_url, name))
        .call()
        .map_err(fail)?;
    match response.status().as_u16() {
        200 => {}
        404 => return Err(DownloadError::NotFound),
        code => return Err(DownloadError::Status(code)),
    }
    if response
        .body()
        .content_length()
        .is_some_and(|n| n > limits.max_bytes)
    {
        return Err(DownloadError::TooLarge(limits.max_bytes));
    }
    response
        .body_mut()
        .with_config()
        .limit(limits.max_bytes)
        .read_to_vec()
        .map_err(fail)
}

/// One-shot HTTP/1.1 servers on 127.0.0.1 for download tests (feature `test-util`).
#[cfg(any(test, feature = "test-util"))]
pub mod test_server {
    #![allow(clippy::expect_used)]

    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    /// Serves one connection on 127.0.0.1: reads the request head, then hands the stream to
    /// `respond`. Returns the base URL and the request path the client sent.
    pub fn serve(
        respond: impl FnOnce(&mut TcpStream) + Send + 'static,
    ) -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}/mapImages", listener.local_addr().expect("addr"));
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut head = Vec::new();
            let mut byte = [0];
            while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).is_ok_and(|n| n == 1) {
                head.push(byte[0]);
            }
            let line = String::from_utf8_lossy(&head);
            let path = line.split(' ').nth(1).unwrap_or_default().to_owned();
            let _ = tx.send(path);
            respond(&mut stream);
            let _ = stream.flush();
        });
        (base, rx)
    }

    /// A 200 response with `body` and its Content-Length.
    pub fn ok(body: Vec<u8>, content_type: &'static str) -> impl FnOnce(&mut TcpStream) + Send {
        move |stream| {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    }

    pub fn not_found(stream: &mut TcpStream) {
        let _ = stream.write_all(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 14\r\nConnection: close\r\n\r\n\
              404: Not Found",
        );
    }

    /// A small valid PNG of `w` x `h`.
    pub fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba(rgba));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)
            .expect("encode");
        out.into_inner()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::io::Write;
    use std::time::{Duration, Instant};

    use super::test_server::{not_found, ok, png, serve};
    use super::*;

    fn fast() -> Limits {
        Limits {
            connect: Duration::from_secs(2),
            global: Duration::from_millis(500),
            max_bytes: 1000,
        }
    }

    #[test]
    fn success_returns_the_body_from_the_map_path() {
        let body = png(4, 4, [1, 2, 3, 255]);
        let (base, path) = serve(ok(body.clone(), "image/png"));
        assert_eq!(fetch_map_image(&base, "M x", fast()), Ok(body));
        assert_eq!(path.recv().expect("path"), "/mapImages/M%20x/M%20x.png");
    }

    #[test]
    fn a_404_is_not_found() {
        let (base, _) = serve(not_found);
        assert_eq!(
            fetch_map_image(&base, "M", fast()),
            Err(DownloadError::NotFound)
        );
    }

    #[test]
    fn another_status_names_it() {
        let (base, _) = serve(|s| {
            let _ = s.write_all(b"HTTP/1.1 500 Oops\r\nContent-Length: 0\r\n\r\n");
        });
        assert_eq!(
            fetch_map_image(&base, "M", fast()),
            Err(DownloadError::Status(500))
        );
    }

    #[test]
    fn a_silent_server_times_out() {
        let (base, _) = serve(|_| std::thread::sleep(Duration::from_secs(3)));
        let start = Instant::now();
        assert_eq!(
            fetch_map_image(&base, "M", fast()),
            Err(DownloadError::Timeout)
        );
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn a_body_stalling_midway_times_out() {
        let (base, _) = serve(|s| {
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 500\r\n\r\nabc");
            let _ = s.flush();
            std::thread::sleep(Duration::from_secs(3));
        });
        assert_eq!(
            fetch_map_image(&base, "M", fast()),
            Err(DownloadError::Timeout)
        );
    }

    #[test]
    fn an_oversize_content_length_is_refused() {
        let (base, _) = serve(ok(vec![0; 1001], "image/png"));
        assert_eq!(
            fetch_map_image(&base, "M", fast()),
            Err(DownloadError::TooLarge(1000))
        );
    }

    #[test]
    fn an_oversize_chunked_body_is_aborted_while_reading() {
        let (base, _) = serve(|s| {
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n");
            for _ in 0..20 {
                let _ = s.write_all(b"64\r\n");
                let _ = s.write_all(&[7; 100]);
                let _ = s.write_all(b"\r\n");
            }
            let _ = s.write_all(b"0\r\n\r\n");
        });
        assert_eq!(
            fetch_map_image(&base, "M", fast()),
            Err(DownloadError::TooLarge(1000))
        );
    }

    #[test]
    fn the_url_is_the_data_repository_layout() {
        assert_eq!(
            map_image_url(MAP_IMAGES_URL, "NF_Marsch_4fach"),
            "https://raw.githubusercontent.com/KillBait/AutoDrive_MapImages/master/mapImages/\
             NF_Marsch_4fach/NF_Marsch_4fach.png"
        );
        assert_eq!(
            map_image_url("http://h/x/", "a/b"),
            "http://h/x/a%2Fb/a%2Fb.png"
        );
        assert_eq!(Limits::default().max_bytes, 32 * 1024 * 1024);
    }
}
