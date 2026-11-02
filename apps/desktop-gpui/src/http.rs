#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use std::{pin::Pin, sync::LazyLock, time::Duration};

use bytes::Bytes;
use futures_util::{Stream, StreamExt};

const ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];

pub(crate) fn validate_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value).map_err(|error| error.to_string())?;
    if url.scheme() != "https"
        || !url
            .host_str()
            .is_some_and(|host| ALLOWED_HOSTS.contains(&host))
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Downloads require HTTPS from an allowed host".to_string());
    }
    Ok(url)
}

fn redirect_url(current: &reqwest::Url, location: &str) -> Result<reqwest::Url, String> {
    let next = current.join(location).map_err(|error| error.to_string())?;
    validate_url(next.as_str())
}

fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .user_agent(crate::brand::USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|error| error.to_string())
    });
    CLIENT.as_ref().map_err(Clone::clone)
}

pub(crate) struct Download {
    status: u16,
    content_length: Option<u64>,
    stream: Pin<Box<dyn Stream<Item = Result<Bytes, String>> + Send>>,
}

impl Download {
    pub(crate) fn status(&self) -> u16 {
        self.status
    }

    pub(crate) fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    pub(crate) fn error_for_status(self) -> Result<Self, String> {
        if (400..600).contains(&self.status) {
            return Err(format!("Download failed: HTTP {}", self.status));
        }
        Ok(self)
    }

    pub(crate) fn bytes_stream(self) -> Pin<Box<dyn Stream<Item = Result<Bytes, String>> + Send>> {
        self.stream
    }

    pub(crate) async fn bytes(mut self) -> Result<Vec<u8>, String> {
        let mut body = Vec::new();
        while let Some(chunk) = self.stream.next().await {
            body.extend_from_slice(&chunk?);
        }
        Ok(body)
    }

    #[cfg(test)]
    pub(crate) fn from_bytes(body: &'static [u8]) -> Self {
        Self {
            status: 200,
            content_length: Some(body.len() as u64),
            stream: Box::pin(futures_util::stream::once(async move {
                Ok(Bytes::from_static(body))
            })),
        }
    }
}

pub(crate) async fn get(url: &str, timeout: Duration) -> Result<Download, String> {
    let mut url = validate_url(url)?;
    let client = client()?;
    let deadline = tokio::time::Instant::now() + timeout;
    for _ in 0..=10 {
        let response = client
            .get(url.clone())
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308)
            && let Some(location) = response.headers().get(reqwest::header::LOCATION)
        {
            url = redirect_url(&url, location.to_str().map_err(|error| error.to_string())?)?;
            continue;
        }
        return Ok(Download {
            status: response.status().as_u16(),
            content_length: response.content_length(),
            stream: Box::pin(
                response
                    .bytes_stream()
                    .map(|chunk| chunk.map_err(|error| error.to_string())),
            ),
        });
    }
    Err("The download redirected too many times".to_string())
}

#[cfg(test)]
mod tests {
    fn credential_url() -> url::Url {
        let mut url = url::Url::parse("https://github.com/model").unwrap();
        url.set_username("example").unwrap();
        url.set_password(Some("example")).unwrap();
        url
    }

    #[test]
    fn refuses_unlisted_download_hosts_and_non_https_urls() {
        let credential_url = credential_url();
        for url in [
            "https://example.com/model.bin",
            "https://github.com.example.com/model.bin",
            "http://github.com/model.bin",
            "https://github.com:444/model.bin",
            credential_url.as_str(),
        ] {
            assert!(super::validate_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn accepts_release_api_and_download_hosts() {
        for url in [
            "https://github.com/AniBullet/FrameEver/releases/latest/download/latest.json",
            "https://api.github.com/repos/AniBullet/FrameEver/releases",
            "https://release-assets.githubusercontent.com/artifact?token=signed",
            "https://objects.githubusercontent.com/artifact",
        ] {
            assert!(super::validate_url(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn redirects_apply_the_same_allowlist() {
        let current =
            super::validate_url("https://github.com/a/b/releases/download/model").unwrap();
        let credential_url = credential_url();
        let relative_credential_url =
            format!("//{}", &credential_url[url::Position::BeforeUsername..]);
        for location in [
            "https://example.com/model",
            "//example.com/model",
            "http://github.com/model",
            "https://github.com:444/model",
            credential_url.as_str(),
            relative_credential_url.as_str(),
        ] {
            assert!(
                super::redirect_url(&current, location).is_err(),
                "{location}"
            );
        }
        assert_eq!(
            super::redirect_url(
                &current,
                "https://release-assets.githubusercontent.com/model"
            )
            .unwrap()
            .host_str(),
            Some("release-assets.githubusercontent.com")
        );
        assert_eq!(
            super::redirect_url(&current, "/next").unwrap().as_str(),
            "https://github.com/next"
        );
    }

    #[test]
    fn requests_refuse_unlisted_hosts_before_connecting() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(super::get(
            "https://127.0.0.1:9/private",
            std::time::Duration::from_secs(1),
        ));
        assert!(result.err().unwrap().contains("allowed host"));
    }

    #[test]
    fn clients_build_without_an_ambient_crypto_provider() {
        assert!(super::client().is_ok());
        assert!(super::client().is_ok());
    }
}
