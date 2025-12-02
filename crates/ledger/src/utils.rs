use base64::{Engine as _, engine::general_purpose};
use rand::Rng;

pub(crate) fn http_to_ws(url: &str) -> String {
    if url.starts_with("ws://") || url.starts_with("wss://") {
        url.to_string()
    } else if url.starts_with("https://") {
        url.replacen("https://", "wss://", 1)
    } else if url.starts_with("http://") {
        url.replacen("http://", "ws://", 1)
    } else {
        url.to_string()
    }
}

pub(crate) fn random_16_byte_string() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill(&mut bytes);
    general_purpose::STANDARD.encode(bytes)
}
