use canton_api_client::apis::configuration::Configuration;

pub struct Client {
    pub(crate) configuration: Configuration,
}

impl Client {
    // Constructor to create a new instance with the given auth token
    pub fn new(auth_token: String, base_url: String) -> Self {
        let configuration = Configuration {
            base_path: base_url,
            bearer_access_token: Some(auth_token),
            ..Configuration::default()
        };

        Client { configuration }
    }
}
