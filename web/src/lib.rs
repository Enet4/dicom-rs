use snafu::Snafu;
mod qido;

#[derive(Debug, Clone)]
struct DicomWebClient {
    wado_url: String,
    qido_url: String,
    stow_url: String,

    // Basic Auth
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    // Bearer Token
    pub(crate) bearer_token: Option<String>,

    pub(crate) client: reqwest::Client,
}

/// An error returned when parsing an invalid tag range.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum DicomWebError {
    #[snafu(display("Failed to perform HTTP request"))]
    RequestFailed { url: String, source: reqwest::Error },
    #[snafu(display("Failed to deserialize response from server"))]
    DeserializationFailed { source: reqwest::Error },
}

impl DicomWebClient {
    pub fn set_basic_auth(&mut self, username: &str, password: &str) -> &Self {
        self.username = Some(username.to_string());
        self.password = Some(password.to_string());
        self
    }

    pub fn set_bearer_token(&mut self, token: &str) -> &Self {
        self.bearer_token = Some(token.to_string());
        self
    }

    pub fn with_single_url(url: &str) -> DicomWebClient {
        DicomWebClient {
            wado_url: url.to_string(),
            qido_url: url.to_string(),
            stow_url: url.to_string(),
            client: reqwest::Client::new(),
            bearer_token: None,
            username: None,
            password: None,
        }
    }

    pub fn with_separate_urls(wado_url: &str, qido_url: &str, stow_url: &str) -> DicomWebClient {
        DicomWebClient {
            wado_url: wado_url.to_string(),
            qido_url: qido_url.to_string(),
            stow_url: stow_url.to_string(),
            client: reqwest::Client::new(),
            bearer_token: None,
            username: None,
            password: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The public Dicomweb server used by OHIF Viewer
    static DICOMWEB_URL: &str = "http://localhost:8042/dicom-web";

    #[tokio::test]
    async fn qido_test() {
        let mut client = DicomWebClient::with_single_url(DICOMWEB_URL);
        client.set_basic_auth("orthanc", "orthanc");

        let result = client.query_studies().run().await;
        assert!(result.is_ok());
        let result = client.query_series().run().await;
        assert!(result.is_ok());
        let result = client.query_instances().run().await;
        assert!(result.is_ok());
        let result = client.query_series_in_study("1.1.1.1").run().await;
        assert!(result.is_ok());
        let result = client
            .query_instances_in_series("1.1.1.1", "1.1.1.1")
            .run()
            .await;
        assert!(result.is_ok());
    }
}
