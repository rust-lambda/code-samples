use lazy_static::lazy_static;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

// This is for DEMONSTRATION purposes only
// Static variables are normally a bad practice in Lambda, as separate execution environments will not share values
lazy_static! {
    static ref SHORTENED_URLS: Mutex<HashMap<String, String>> = {
        let mut m = HashMap::new();
        Mutex::new(m)
    };
}

#[derive(Deserialize)]
pub struct ShortenUrlRequest {
    url_to_shorten: String,
}

#[derive(Serialize)]
pub struct ShortenUrlResponse {
    shortened_url: String,
}

pub struct UrlShortener {}

impl UrlShortener {
    pub fn new() -> Self {
        Self {}
    }

    pub fn shorten_url(&self, req: ShortenUrlRequest) -> Result<ShortenUrlResponse, ()> {
        let short_url = self.generate_short_url();

        let mut map = SHORTENED_URLS.lock().unwrap();
        map.insert(short_url.clone(), req.url_to_shorten);

        Ok(ShortenUrlResponse {
            shortened_url: short_url,
        })
    }

    pub fn retrieve_url(&self, short_url: String) -> Option<String> {
        let map = SHORTENED_URLS.lock().unwrap();
        match map.get(&short_url) {
            None => None,
            Some(url) => Some(url.clone()),
        }
    }

    fn generate_short_url(&self) -> String {
        let mut rng = thread_rng();
        (0..8).map(|_| rng.sample(Alphanumeric) as char).collect()
    }
}
