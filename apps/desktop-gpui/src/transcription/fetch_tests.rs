use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use futures_util::future::BoxFuture;
use futures_util::stream::{self, StreamExt};

use super::fetch::{self, Part, Patience, Progress};
use crate::http::Download;

const BODY: &[u8] = b"0123456789";

#[derive(Clone, Copy)]
enum Reply {
    Drop(usize),
    Stall(usize),
    Honour,
    Ignore,
    NotFound,
}

struct Server {
    replies: Mutex<VecDeque<Reply>>,
    ranges: Mutex<Vec<u64>>,
}

impl Server {
    fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            ranges: Mutex::new(Vec::new()),
        })
    }

    fn ranges(&self) -> Vec<u64> {
        self.ranges.lock().unwrap().clone()
    }

    fn opener(
        self: &Arc<Self>,
    ) -> impl Fn(String, u64) -> BoxFuture<'static, Result<Download, String>> + Send + Sync + 'static
    {
        let server = self.clone();
        move |_url, from| {
            server.ranges.lock().unwrap().push(from);
            let reply = server
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("a reply for every request");
            Box::pin(async move { Ok(respond(reply, from as usize)) })
        }
    }
}

fn chunks(body: &'static [u8]) -> impl futures_util::Stream<Item = Result<Bytes, String>> {
    stream::iter(
        body.chunks(2)
            .map(|chunk| Ok(Bytes::from_static(chunk)))
            .collect::<Vec<_>>(),
    )
}

fn respond(reply: Reply, from: usize) -> Download {
    match reply {
        Reply::Drop(n) => Download::scripted(
            200,
            Box::pin(
                chunks(&BODY[..n])
                    .chain(stream::once(async { Err("connection reset".to_string()) })),
            ),
        ),
        Reply::Stall(n) => {
            Download::scripted(200, Box::pin(chunks(&BODY[..n]).chain(stream::pending())))
        }
        Reply::Honour if from > 0 => Download::scripted(206, Box::pin(chunks(&BODY[from..]))),
        Reply::Honour | Reply::Ignore => Download::scripted(200, Box::pin(chunks(BODY))),
        Reply::NotFound => Download::scripted(404, Box::pin(stream::empty())),
    }
}

fn quick() -> Patience {
    Patience {
        stall: Duration::from_millis(300),
        attempts: 4,
        backoff: Duration::from_millis(5),
    }
}

async fn fetch_into(
    server: &Arc<Server>,
    patience: Patience,
) -> (Vec<u8>, Result<u64, String>, i64) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.bin.downloading");
    let (mut file, have) = fetch::reopen(&path).unwrap();
    let mut counted = 0i64;
    let mut progress = |event: Progress<'_>| match event {
        Progress::Bytes(chunk) => counted += chunk.len() as i64,
        Progress::Rewound(bytes) => counted -= bytes as i64,
    };
    let open = server.opener();
    let result = fetch::part(
        &open,
        Part {
            url: "https://github.com/model.bin",
            base: 0,
            have,
        },
        &mut file,
        patience,
        &mut progress,
    )
    .await;
    drop(file);
    (std::fs::read(&path).unwrap(), result, counted)
}

#[tokio::test]
async fn a_dropped_download_resumes_where_it_stopped() {
    let server = Server::new(vec![Reply::Drop(4), Reply::Honour]);
    let (bytes, result, counted) = fetch_into(&server, quick()).await;
    assert_eq!(bytes, BODY);
    assert_eq!(result.unwrap(), 10);
    assert_eq!(counted, 10);
    assert_eq!(server.ranges(), vec![0, 4]);
}

#[tokio::test]
async fn a_server_that_ignores_the_range_writes_the_part_again() {
    let server = Server::new(vec![Reply::Drop(4), Reply::Ignore]);
    let (bytes, result, counted) = fetch_into(&server, quick()).await;
    assert_eq!(bytes, BODY, "the first 4 bytes must not be kept twice");
    assert_eq!(result.unwrap(), 10);
    assert_eq!(counted, 10);
    assert_eq!(server.ranges(), vec![0, 4]);
}

#[tokio::test]
async fn a_stalled_download_is_tried_again_from_where_it_stood() {
    let server = Server::new(vec![Reply::Stall(4), Reply::Honour]);
    let (bytes, result, _) = fetch_into(&server, quick()).await;
    assert_eq!(bytes, BODY);
    assert_eq!(result.unwrap(), 10);
    assert_eq!(server.ranges(), vec![0, 4]);
}

#[tokio::test]
async fn a_missing_file_fails_at_once_without_retries() {
    let server = Server::new(vec![Reply::NotFound, Reply::Honour]);
    let (_, result, _) = fetch_into(&server, quick()).await;
    assert!(result.unwrap_err().contains("404"));
    assert_eq!(server.ranges().len(), 1);
}

#[tokio::test]
async fn a_download_that_keeps_dropping_gives_up_after_its_tries() {
    let server = Server::new(vec![Reply::Drop(2), Reply::Drop(2), Reply::Drop(2)]);
    let patience = Patience {
        attempts: 3,
        ..quick()
    };
    let (_, result, _) = fetch_into(&server, patience).await;
    assert!(result.is_err());
    assert_eq!(server.ranges().len(), 3);
}

#[test]
fn sizes_read_as_people_say_them() {
    assert_eq!(fetch::size_label(1), "1 MB");
    assert_eq!(fetch::size_label(420_000_000), "420 MB");
    assert_eq!(fetch::size_label(1_000_000_000), "1.0 GB");
    assert_eq!(fetch::size_label(1_234_000_000), "1.3 GB");
}

#[tokio::test]
async fn a_one_part_whisper_model_resumes_its_staging_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("base.bin");
    std::fs::write(super::whisper_staging_path(&path), &BODY[..6]).unwrap();
    let server = Server::new(vec![Reply::Honour]);
    let open = server.opener();
    super::write_whisper_parts(&open, "base", &["https://github.com/base.bin"], &path, 10)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), BODY);
    assert_eq!(server.ranges(), vec![6]);
    assert!(!super::whisper_staging_path(&path).exists());
}

#[tokio::test]
async fn a_whisper_model_of_the_wrong_size_is_never_installed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("base.bin");
    let server = Server::new(vec![Reply::Honour]);
    let open = server.opener();
    let result =
        super::write_whisper_parts(&open, "base", &["https://github.com/base.bin"], &path, 12)
            .await;
    assert!(result.is_err());
    assert!(!path.exists());
    assert!(!super::whisper_staging_path(&path).exists());
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
#[test]
fn only_staged_files_no_longer_than_expected_resume() {
    let dir = tempfile::tempdir().unwrap();
    let files: &[(&str, &[&str])] = &[
        ("a.onnx", &["https://example.invalid/a.onnx"]),
        ("b.onnx", &["https://example.invalid/b.onnx"]),
        (
            "c.onnx",
            &[
                "https://example.invalid/c.part0",
                "https://example.invalid/c.part1",
            ],
        ),
        ("d.onnx", &["https://example.invalid/d.onnx"]),
    ];
    std::fs::write(dir.path().join("a.onnx"), b"1234").unwrap();
    std::fs::write(dir.path().join("b.onnx"), b"123456789").unwrap();
    std::fs::write(dir.path().join("c.onnx"), b"12").unwrap();
    let sizes = [
        ("a.onnx", 10),
        ("b.onnx", 5),
        ("c.onnx", 10),
        ("d.onnx", 10),
    ];
    assert_eq!(
        super::resumable_parakeet_bytes(dir.path(), files, &sizes),
        vec![4, 0, 0, 0]
    );
}
