use std::sync::Once;

pub fn client_builder() -> reqwest::ClientBuilder {
    static CRYPTO_PROVIDER: Once = Once::new();
    CRYPTO_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    reqwest::Client::builder()
}

#[cfg(test)]
mod tests {
    #[test]
    fn clients_build_without_an_ambient_crypto_provider() {
        assert!(super::client_builder().build().is_ok());
        assert!(super::client_builder().build().is_ok());
    }
}
