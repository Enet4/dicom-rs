use dicom_json::DicomJson;
use dicom_object::{from_reader, FileDicomObject, InMemDicomObject};

use futures_util::{Stream, StreamExt};
use multipart_rs::{MultipartItem, MultipartReader, MultipartType};
use snafu::{OptionExt, ResultExt};

use crate::{
    DeserializationFailedSnafu, DicomReaderFailedSnafu, DicomWebClient, DicomWebError,
    EmptyResponseSnafu, MissingContentTypeHeaderSnafu, MultipartReaderFailedSnafu,
    RequestFailedSnafu,
};

#[derive(Debug, Clone)]
pub struct WadoMetadataRequest {
    client: DicomWebClient,
    url: String,
}

impl WadoMetadataRequest {
    pub fn new(client: DicomWebClient, url: String) -> Self {
        WadoMetadataRequest { client, url }
    }

    pub async fn run(&self) -> Result<Vec<InMemDicomObject>, DicomWebError> {
        let mut request = self.client.client.get(&self.url);

        // Basic authentication
        if let Some(username) = &self.client.username {
            request = request.basic_auth(username, self.client.password.as_ref());
        }
        // Bearer token
        else if let Some(bearer_token) = &self.client.bearer_token {
            request = request.bearer_auth(bearer_token);
        }

        let response = request
            .send()
            .await
            .context(RequestFailedSnafu { url: &self.url })?;

        if !response.status().is_success() {
            return Err(DicomWebError::HttpStatusFailure {
                status_code: response.status(),
            });
        }

        // Check if the response is a DICOM-JSON
        if let Some(ct) = response.headers().get("Content-Type") {
            if ct != "application/dicom+json" {
                return Err(DicomWebError::UnexpectedContentType {
                    content_type: ct.to_str().unwrap_or_default().to_string(),
                });
            }
        } else {
            return Err(DicomWebError::MissingContentTypeHeader);
        }

        Ok(response
            .json::<Vec<DicomJson<InMemDicomObject>>>()
            .await
            .context(DeserializationFailedSnafu {})?
            .into_iter()
            .map(|dj| dj.into_inner())
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct WadoFileRequest {
    client: DicomWebClient,
    url: String,
}

impl WadoFileRequest {
    pub fn new(client: DicomWebClient, url: String) -> Self {
        WadoFileRequest { client, url }
    }

    pub async fn run(
        &self,
    ) -> Result<
        impl Stream<Item = Result<FileDicomObject<InMemDicomObject>, DicomWebError>>,
        DicomWebError,
    > {
        let mut request = self.client.client.get(&self.url);

        // Basic authentication
        if let Some(username) = &self.client.username {
            request = request.basic_auth(username, self.client.password.as_ref());
        }
        // Bearer token
        else if let Some(bearer_token) = &self.client.bearer_token {
            request = request.bearer_auth(bearer_token);
        }

        let response = request
            .send()
            .await
            .context(RequestFailedSnafu { url: &self.url })?;

        if !response.status().is_success() {
            return Err(DicomWebError::HttpStatusFailure {
                status_code: response.status(),
            });
        }

        // Build the MultipartReader
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), String::from(v.to_str().unwrap_or_default())))
            .collect();

        let stream = response.bytes_stream();
        let reader = MultipartReader::from_stream_with_headers(stream, &headers)
            .map_err(|source| DicomWebError::MultipartReaderFailed { source })?;

        if reader.multipart_type != MultipartType::Related {
            return Err(DicomWebError::UnexpectedMultipartType {
                multipart_type: (reader.multipart_type),
            });
        }

        Ok(reader.map(|item| {
            let item = item.context(MultipartReaderFailedSnafu)?;
            // Get the Content-Type header
            let content_type = item
                .headers
                .iter()
                .find(|(k, _)| k.to_lowercase() == "content-type")
                .map(|(_, v)| v.as_str())
                .context(MissingContentTypeHeaderSnafu)?;

            if content_type != "application/dicom" {
                return Err(DicomWebError::UnexpectedContentType {
                    content_type: content_type.to_string(),
                });
            }

            from_reader(&*item.data).context(DicomReaderFailedSnafu)
        }))
    }
}

pub struct WadoSingleFileRequest {
    request: WadoFileRequest,
}

impl WadoSingleFileRequest {
    pub async fn run(&self) -> Result<FileDicomObject<InMemDicomObject>, DicomWebError> {
        // Run the request and get the first item of the stream
        let mut stream = self.request.run().await?;
        stream.next().await.context(EmptyResponseSnafu)?
    }
}

pub struct WadoFramesRequest {
    client: DicomWebClient,
    url: String,
}

impl WadoFramesRequest {
    pub fn new(client: DicomWebClient, url: String) -> Self {
        WadoFramesRequest { client, url }
    }

    pub async fn run(&self) -> Result<Vec<MultipartItem>, DicomWebError> {
        let mut request = self.client.client.get(&self.url);

        // Basic authentication
        if let Some(username) = &self.client.username {
            request = request.basic_auth(username, self.client.password.as_ref());
        }
        // Bearer token
        else if let Some(bearer_token) = &self.client.bearer_token {
            request = request.bearer_auth(bearer_token);
        }

        let response = request
            .send()
            .await
            .context(RequestFailedSnafu { url: &self.url })?;

        if !response.status().is_success() {
            return Err(DicomWebError::HttpStatusFailure {
                status_code: response.status(),
            });
        }

        // Build the MultipartReader
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), String::from(v.to_str().unwrap_or_default())))
            .collect();
        let stream = response.bytes_stream();
        let mut reader = MultipartReader::from_stream_with_headers(stream, &headers)
            .map_err(|source| DicomWebError::MultipartReaderFailed { source })?;

        if reader.multipart_type != MultipartType::Related {
            return Err(DicomWebError::UnexpectedMultipartType {
                multipart_type: (reader.multipart_type),
            });
        }

        let mut item_list = vec![];

        while let Some(item) = reader.next().await {
            let item = item.context(MultipartReaderFailedSnafu)?;
            item_list.push(item);
        }

        Ok(item_list)
    }
}

impl DicomWebClient {
    pub fn retrieve_study(&self, study_instance_uid: &str) -> WadoFileRequest {
        let url = format!("{}/studies/{}", self.wado_url, study_instance_uid);
        WadoFileRequest::new(self.clone(), url)
    }

    pub fn retrieve_study_metadata(&self, study_instance_uid: &str) -> WadoMetadataRequest {
        let url = format!("{}/studies/{}/metadata", self.wado_url, study_instance_uid);
        WadoMetadataRequest::new(self.clone(), url)
    }

    pub fn retrieve_series(
        &self,
        study_instance_uid: &str,
        series_instance_uid: &str,
    ) -> WadoFileRequest {
        let base_url = &self.wado_url;
        let url = format!("{base_url}/studies/{study_instance_uid}/series/{series_instance_uid}",);
        WadoFileRequest::new(self.clone(), url)
    }

    pub fn retrieve_series_metadata(
        &self,
        study_instance_uid: &str,
        series_instance_uid: &str,
    ) -> WadoMetadataRequest {
        let base_url = &self.wado_url;
        let url = format!(
            "{base_url}/studies/{study_instance_uid}/series/{series_instance_uid}/metadata"
        );
        WadoMetadataRequest::new(self.clone(), url)
    }

    pub fn retrieve_instance(
        &self,
        study_instance_uid: &str,
        series_instance_uid: &str,
        sop_instance_uid: &str,
    ) -> WadoSingleFileRequest {
        let base_url = &self.wado_url;
        let url = format!(
            "{base_url}/studies/{study_instance_uid}/series/{series_instance_uid}/instances/{sop_instance_uid}",
        );
        WadoSingleFileRequest {
            request: WadoFileRequest::new(self.clone(), url),
        }
    }

    pub fn retrieve_instance_metadata(
        &self,
        study_instance_uid: &str,
        series_instance_uid: &str,
        sop_instance_uid: &str,
    ) -> WadoMetadataRequest {
        let base_url = &self.wado_url;
        let url = format!(
            "{base_url}/studies/{study_instance_uid}/series/{series_instance_uid}/instances/{sop_instance_uid}/metadata",
        );
        WadoMetadataRequest::new(self.clone(), url)
    }

    pub fn retrieve_frames(
        &self,
        study_instance_uid: &str,
        series_instance_uid: &str,
        sop_instance_uid: &str,
        framelist: &[u32],
    ) -> WadoFramesRequest {
        let framelist = framelist
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<String>>()
            .join(",");
        let base_url = &self.wado_url;
        let url = format!(
            "{base_url}/studies/{study_instance_uid}/series/{series_instance_uid}/instances/{sop_instance_uid}/frames/{framelist}",
        );
        WadoFramesRequest::new(self.clone(), url)
    }
}
