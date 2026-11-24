use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::future::BoxFuture;

use crate::http::Download;

pub(crate) type Opener =
    dyn Fn(String, u64) -> BoxFuture<'static, Result<Download, String>> + Send + Sync;

pub(crate) fn network(url: String, from: u64) -> BoxFuture<'static, Result<Download, String>> {
    Box::pin(async move { crate::http::get_from(&url, from).await })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Patience {
    pub(crate) stall: Duration,
    pub(crate) attempts: u32,
    pub(crate) backoff: Duration,
}

impl Default for Patience {
    fn default() -> Self {
        Self {
            stall: Duration::from_secs(60),
            attempts: 5,
            backoff: Duration::from_secs(2),
        }
    }
}

pub(crate) enum Progress<'a> {
    Bytes(&'a [u8]),
    Rewound(u64),
}

enum Failure {
    Retry(String),
    Stop(String),
}

pub(crate) struct Part<'a> {
    pub(crate) url: &'a str,
    pub(crate) base: u64,
    pub(crate) have: u64,
}

pub(crate) async fn part(
    open: &Opener,
    part: Part<'_>,
    file: &mut File,
    patience: Patience,
    progress: &mut (dyn FnMut(Progress<'_>) + Send),
) -> Result<u64, String> {
    let Part {
        url,
        base,
        mut have,
    } = part;
    let mut attempt = 0;
    loop {
        attempt += 1;
        match once(open, url, file, base, &mut have, patience.stall, progress).await {
            Ok(()) => return Ok(have),
            Err(Failure::Stop(error)) => return Err(error),
            Err(Failure::Retry(error)) => {
                if attempt >= patience.attempts {
                    return Err(error);
                }
                tracing::warn!(url, attempt, have, %error, "model download interrupted, resuming");
                tokio::time::sleep(patience.backoff * attempt).await;
            }
        }
    }
}

async fn once(
    open: &Opener,
    url: &str,
    file: &mut File,
    base: u64,
    have: &mut u64,
    stall: Duration,
    progress: &mut (dyn FnMut(Progress<'_>) + Send),
) -> Result<(), Failure> {
    let response = match tokio::time::timeout(stall, open(url.to_string(), *have)).await {
        Err(_) => return Err(Failure::Retry("the server did not answer".into())),
        Ok(Err(error)) => return Err(Failure::Retry(error)),
        Ok(Ok(response)) => response,
    };
    let status = response.status();
    if *have > 0 && status == 416 {
        rewind(file, base, have, progress)?;
        return Err(Failure::Retry(format!("HTTP {status}")));
    }
    if (500..600).contains(&status) || status == 408 || status == 429 {
        return Err(Failure::Retry(format!("HTTP {status}")));
    }
    if !(200..300).contains(&status) {
        return Err(Failure::Stop(format!("HTTP {status}")));
    }
    if *have > 0 && status != 206 {
        rewind(file, base, have, progress)?;
    }
    let mut stream = response.bytes_stream();
    loop {
        let chunk = match tokio::time::timeout(stall, stream.next()).await {
            Err(_) => return Err(Failure::Retry("the download stalled".into())),
            Ok(None) => return Ok(()),
            Ok(Some(Err(error))) => return Err(Failure::Retry(error)),
            Ok(Some(Ok(chunk))) => chunk,
        };
        file.write_all(&chunk)
            .map_err(|e| Failure::Stop(format!("Error while writing to file: {e}")))?;
        *have += chunk.len() as u64;
        progress(Progress::Bytes(&chunk));
    }
}

fn rewind(
    file: &mut File,
    base: u64,
    have: &mut u64,
    progress: &mut (dyn FnMut(Progress<'_>) + Send),
) -> Result<(), Failure> {
    file.set_len(base)
        .and_then(|()| file.seek(SeekFrom::Start(base)).map(|_| ()))
        .map_err(|e| Failure::Stop(format!("Error while writing to file: {e}")))?;
    progress(Progress::Rewound(*have));
    *have = 0;
    Ok(())
}

pub(crate) fn reopen(path: &Path) -> std::io::Result<(File, u64)> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    let len = file.seek(SeekFrom::End(0))?;
    Ok((file, len))
}

pub(crate) fn restart(file: &mut File) -> std::io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0)).map(|_| ())
}

const SPACE_HEADROOM: u64 = 512 * 1024 * 1024;

pub(crate) fn missing_space(dir: &Path, bytes: u64) -> Option<u64> {
    let free = frameever_utils::disk_space::free_bytes_for_path(dir).ok()?;
    let needed = bytes.saturating_add(SPACE_HEADROOM);
    (free < needed).then(|| needed - free)
}

pub(crate) fn size_label(bytes: u64) -> String {
    let tenths = bytes.div_ceil(100_000_000).max(1);
    if tenths >= 10 {
        format!("{}.{} GB", tenths / 10, tenths % 10)
    } else {
        format!("{} MB", (bytes.div_ceil(1_000_000)).max(1))
    }
}
