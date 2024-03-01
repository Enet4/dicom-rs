use dicom_json::DicomJson;
use dicom_object::{from_reader, FileDicomObject, InMemDicomObject};

use futures_util::StreamExt;
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

    pub async fn run(self: &Self) -> Result<Vec<InMemDicomObject>, DicomWebError> {
        let request = self.client.client.get(&self.url);

        // Basic authentication
        let request = if self.client.username.is_some() {
            request.basic_auth(
                self.client.username.as_ref().unwrap().to_string(),
                self.client.password.as_ref(),
            )
        } else {
            request
        };
        // Bearer token
        let request = if self.client.bearer_token.is_some() {
            request.bearer_auth(self.client.bearer_token.as_ref().unwrap())
        } else {
            request
        };

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
        let ct = response.headers().get("Content-Type");
        if ct.is_none() {
            return Err(DicomWebError::MissingContentTypeHeader);
        }

        if ct.unwrap() != "application/dicom+json" {
            return Err(DicomWebError::UnexpectedContentType {
                content_type: ct.unwrap().to_str().unwrap().to_string(),
            });
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

    pub async fn run(self: &Self) -> Result<Vec<FileDicomObject<InMemDicomObject>>, DicomWebError> {
        let request = self.client.client.get(&self.url);

        // Basic authentication
        let request = if self.client.username.is_some() {
            request.basic_auth(
                self.client.username.as_ref().unwrap().to_string(),
                self.client.password.as_ref(),
            )
        } else {
            request
        };
        // Bearer token
        let request = if self.client.bearer_token.is_some() {
            request.bearer_auth(self.client.bearer_token.as_ref().unwrap())
        } else {
            request
        };

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
            .map(|(k, v)| (k.to_string(), String::from(v.to_str().unwrap_or(""))))
            .collect();
        println!("{:?}", headers);
        let stream = response.bytes_stream();
        let mut reader = MultipartReader::from_stream_with_headers(stream, &headers)
            .map_err(|source| DicomWebError::MultipartReaderFailed { source })?;

        if reader.multipart_type != MultipartType::Related {
            return Err(DicomWebError::UnexpectedMultipartType {
                multipart_type: (reader.multipart_type),
            });
        }

        let mut dcm_list = vec![];

        while let Some(file) = reader.next().await {
            let file = file.context(MultipartReaderFailedSnafu)?;
            // Get the Content-Type header
            let content_type = file
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

            let dcm = from_reader(&*file.data).context(DicomReaderFailedSnafu)?;
            dcm_list.push(dcm);
        }

        Ok(dcm_list)
    }
}

pub struct WadoSingleFileRequest {
    request: WadoFileRequest,
}

impl WadoSingleFileRequest {
    pub async fn run(self: &Self) -> Result<FileDicomObject<InMemDicomObject>, DicomWebError> {
        return self
            .request
            .run()
            .await
            .map(|mut v| v.pop().context(EmptyResponseSnafu))?;
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

    pub async fn run(self: &Self) -> Result<Vec<MultipartItem>, DicomWebError> {
        let request = self.client.client.get(&self.url);

        // Basic authentication
        let request = if self.client.username.is_some() {
            request.basic_auth(
                self.client.username.as_ref().unwrap().to_string(),
                self.client.password.as_ref(),
            )
        } else {
            request
        };
        // Bearer token
        let request = if self.client.bearer_token.is_some() {
            request.bearer_auth(self.client.bearer_token.as_ref().unwrap())
        } else {
            request
        };

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
            .map(|(k, v)| (k.to_string(), String::from(v.to_str().unwrap_or(""))))
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
