use multipart_rs::MultipartType;
use reqwest::StatusCode;
use snafu::Snafu;

mod qido;
mod wado;

#[derive(Debug, Clone)]
pub struct DicomWebClient {
    wado_url: String,
    qido_url: String,
    _stow_url: String,

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
    #[snafu(display("Failed to parse multipart response"))]
    MultipartReaderFailed {
        source: multipart_rs::MultipartError,
    },
    #[snafu(display("Failed to read DICOM object from multipart item"))]
    DicomReaderFailed { source: dicom_object::ReadError },
    #[snafu(display("HTTP status code indicates failure"))]
    HttpStatusFailure { status_code: StatusCode },
    #[snafu(display("Multipart item missing Content-Type header"))]
    MissingContentTypeHeader,
    #[snafu(display("Unexpected content type: {}", content_type))]
    UnexpectedContentType { content_type: String },
    #[snafu(display("Failed to parse content type: {}", source))]
    ContentTypeParseFailed { source: mime::FromStrError },
    #[snafu(display("Unexpected multipart type: {:?}", multipart_type))]
    UnexpectedMultipartType { multipart_type: MultipartType },
    #[snafu(display("Empty response"))]
    EmptyResponse,
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
            _stow_url: url.to_string(),
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
            _stow_url: stow_url.to_string(),
            client: reqwest::Client::new(),
            bearer_token: None,
            username: None,
            password: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::MockServer;

    use super::*;

    async fn mock_qido(mock_server: &MockServer) {
        // STUDIES endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path("/studies"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!([])));
        mock_server.register(mock).await;
        // SERIES endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path("/series"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!([])));
        mock_server.register(mock).await;
        // INSTANCES endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path("/instances"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!([])));
        mock_server.register(mock).await;
        // STUDIES/{STUDY_UID}/SERIES endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path_regex("^/studies/[0-9.]+/series$"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!([])));
        mock_server.register(mock).await;
        // STUDIES/{STUDY_UID}/SERIES/{SERIES_UID}/INSTANCES endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path_regex(
                "^/studies/[0-9.]+/series/[0-9.]+/instances$",
            ))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!([])));
        mock_server.register(mock).await;
    }

    async fn mock_wado(mock_server: &MockServer) {
        let dcm_multipart_response = wiremock::ResponseTemplate::new(200).set_body_raw(
            "--1234\r\nContent-Type: application/dicom\r\n\r\n--1234--",
            "multipart/related; boundary=1234",
        );

        // STUDIES/{STUDY_UID} endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path_regex("^/studies/[0-9.]+$"))
            .respond_with(dcm_multipart_response.clone());
        mock_server.register(mock).await;
        // STUDIES/{STUDY_UID}/METADATA endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path_regex(
                "^/studies/[0-9.]+/metadata$",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw("[]", "application/dicom+json"),
            );
        mock_server.register(mock).await;
        // STUDIES/{STUDY_UID}/SERIES/{SERIES_UID} endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path_regex(
                r"^/studies/[0-9.]+/series/[0-9.]+$",
            ))
            .respond_with(dcm_multipart_response.clone());
        mock_server.register(mock).await;
        // STUDIES/{STUDY_UID}/SERIES/{SERIES_UID}/METADATA endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path_regex(
                r"^/studies/[0-9.]+/series/[0-9.]+/metadata$",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw("[]", "application/dicom+json"),
            );
        mock_server.register(mock).await;
        // STUDIES/{STUDY_UID}/SERIES/{SERIES_UID}/INSTANCES/{INSTANCE_UID} endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path_regex(
                r"^/studies/[0-9.]+/series/[0-9.]+/instances/[0-9.]+$",
            ))
            .respond_with(dcm_multipart_response.clone());
        mock_server.register(mock).await;
        // STUDIES/{STUDY_UID}/SERIES/{SERIES_UID}/INSTANCES/{INSTANCE_UID}/METADATA endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path_regex(
                r"^/studies/[0-9.]+/series/[0-9.]+/instances/[0-9.]+/metadata$",
            ))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw("[]", "application/dicom+json"),
            );
        mock_server.register(mock).await;
        // STUDIES/{STUDY_UID}/SERIES/{SERIES_UID}/INSTANCES/{INSTANCE_UID}/frames/{framelist} endpoint
        let mock = wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header_exists("Accept"))
            .and(wiremock::matchers::path_regex(
                r"^/studies/[0-9.]+/series/[0-9.]+/instances/[0-9.]+/frames/[0-9,]+$",
            ))
            .respond_with(dcm_multipart_response);
        mock_server.register(mock).await;
    }

    // Create a DICOMWeb mock server
    async fn start_dicomweb_mock_server() -> MockServer {
        let mock_server = MockServer::start().await;
        mock_qido(&mock_server).await;
        mock_wado(&mock_server).await;
        mock_server
    }

    #[tokio::test]
    async fn query_study_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform QIDO-RS request
        let result = client.query_studies().run().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn query_series_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform QIDO-RS request
        let result = client.query_series().run().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn query_instances_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform QIDO-RS request
        let result = client.query_instances().run().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn query_series_in_study_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform QIDO-RS request
        let result = client
            .query_series_in_study("1.2.276.0.89.300.10035584652.20181014.93645")
            .run()
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn query_instances_in_series_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform QIDO-RS request
        let result = client
            .query_instances_in_series("1.2.276.0.89.300.10035584652.20181014.93645", "1.1.1.1")
            .run()
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retrieve_study_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform WADO-RS request
        let result = client
            .retrieve_study("1.2.276.0.89.300.10035584652.20181014.93645")
            .run()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retrieve_study_metadata_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform WADO-RS request
        let result = client
            .retrieve_study_metadata("1.2.276.0.89.300.10035584652.20181014.93645")
            .run()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retrieve_series_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform WADO-RS request
        let result = client
            .retrieve_series(
                "1.2.276.0.89.300.10035584652.20181014.93645",
                "1.2.392.200036.9125.3.1696751121028.64888163108.42362053",
            )
            .run()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retrieve_series_metadata_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform WADO-RS request
        let result = client
            .retrieve_series_metadata(
                "1.2.276.0.89.300.10035584652.20181014.93645",
                "1.2.392.200036.9125.3.1696751121028.64888163108.42362053",
            )
            .run()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retrieve_instance_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform WADO-RS request
        let result = client
            .retrieve_instance(
                "1.2.276.0.89.300.10035584652.20181014.93645",
                "1.2.392.200036.9125.3.1696751121028.64888163108.42362053",
                "1.2.392.200036.9125.9.0.454007928.521494544.1883970570",
            )
            .run()
            .await;
        assert!(result.is_err_and(|e| e.to_string().contains("Empty")));
    }

    #[tokio::test]
    async fn retrieve_instance_metadata_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let client = DicomWebClient::with_single_url(&mock_server.uri());
        // Perform WADO-RS request
        let result = client
            .retrieve_instance_metadata(
                "1.2.276.0.89.300.10035584652.20181014.93645",
                "1.2.392.200036.9125.3.1696751121028.64888163108.42362053",
                "1.2.392.200036.9125.9.0.454007928.521494544.1883970570",
            )
            .run()
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn retrieve_frames_test() {
        let mock_server = start_dicomweb_mock_server().await;
        let mut client = DicomWebClient::with_single_url(&mock_server.uri());
        client.set_basic_auth("orthanc", "orthanc");
        // Perform WADO-RS request
        let result = client
            .retrieve_frames(
                "1.2.276.0.89.300.10035584652.20181014.93645",
                "1.2.392.200036.9125.3.1696751121028.64888163108.42362053",
                "1.2.392.200036.9125.9.0.454007928.521494544.1883970570",
                &[1],
            )
            .run()
            .await;
        assert!(result.is_ok());
    }
}
