use dicom_core::Tag;
use dicom_json::DicomJson;
use dicom_object::InMemDicomObject;

use snafu::ResultExt;

use crate::{DeserializationFailedSnafu, DicomWebClient, DicomWebError, RequestFailedSnafu};

#[derive(Debug, Clone)]
pub struct QidoRequest {
    client: DicomWebClient,
    url: String,

    limit: Option<u32>,
    offset: Option<u32>,
    includefields: Vec<Tag>,
    fuzzymatching: Option<bool>,
    filters: Vec<(Tag, String)>,
}

impl QidoRequest {
    pub fn new(client: DicomWebClient, url: String) -> Self {
        QidoRequest {
            client,
            url,
            limit: None,
            offset: None,
            includefields: vec![],
            fuzzymatching: None,
            filters: vec![],
        }
    }

    pub async fn run(&self) -> Result<Vec<InMemDicomObject>, DicomWebError> {
        let mut query: Vec<(String, String)> = vec![];
        if let Some(limit) = self.limit {
            query.push((String::from("limit"), limit.to_string()));
        }
        if let Some(offset) = self.offset {
            query.push((String::from("offset"), offset.to_string()));
        }
        for include_field in self.includefields.iter() {
            // Convert the tag to a radix string
            let radix_string = format!(
                "{:04x}{:04x}",
                include_field.group(),
                include_field.element()
            );

            query.push((String::from("includefield"), radix_string));
        }
        for filter in self.filters.iter() {
            query.push((filter.0.to_string(), filter.1.clone()));
        }

        let mut request = self.client.client.get(&self.url).query(&query);

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
            if ct != "application/dicom+json" && ct != "application/json" {
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

    pub fn with_limit(&mut self, limit: u32) -> &mut Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(&mut self, offset: u32) -> &mut Self {
        self.offset = Some(offset);
        self
    }

    pub fn with_includefields(&mut self, includefields: Vec<Tag>) -> &mut Self {
        self.includefields = includefields;
        self
    }

    pub fn with_fuzzymatching(&mut self, fuzzymatching: bool) -> &mut Self {
        self.fuzzymatching = Some(fuzzymatching);
        self
    }

    pub fn with_filter(&mut self, tag: Tag, value: String) -> &mut Self {
        self.filters.push((tag, value));
        self
    }
}

impl DicomWebClient {
    pub fn query_studies(&self) -> QidoRequest {
        let base_url = &self.qido_url;
        let url = format!("{base_url}/studies");

        QidoRequest::new(self.clone(), url)
    }

    pub fn query_series(&self) -> QidoRequest {
        let base_url = &self.qido_url;
        let url = format!("{base_url}/series");

        QidoRequest::new(self.clone(), url)
    }

    pub fn query_series_in_study(&self, study_instance_uid: &str) -> QidoRequest {
        let base_url = &self.qido_url;
        let url = format!("{base_url}/studies/{study_instance_uid}/series");

        QidoRequest::new(self.clone(), url)
    }

    pub fn query_instances(&self) -> QidoRequest {
        let base_url = &self.qido_url;
        let url = format!("{base_url}/instances");

        QidoRequest::new(self.clone(), url)
    }

    pub fn query_instances_in_series(
        &self,
        study_instance_uid: &str,
        series_instance_uid: &str,
    ) -> QidoRequest {
        let base_url = &self.qido_url;
        let url = format!(
            "{base_url}/studies/{study_instance_uid}/series/{series_instance_uid}/instances",
        );

        QidoRequest::new(self.clone(), url)
    }
}
