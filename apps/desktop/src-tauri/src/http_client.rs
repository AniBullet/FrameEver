use std::ops::Deref;

pub struct HttpClient(reqwest::Client);

impl Default for HttpClient {
    fn default() -> Self {
        Self(reqwest::Client::new())
    }
}

impl Deref for HttpClient {
    type Target = reqwest::Client;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
