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
    includefields: Option<String>,
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
            includefields: None,
            fuzzymatching: None,
            filters: vec![],
        }
    }

    pub async fn run(self: &Self) -> Result<Vec<InMemDicomObject>, DicomWebError> {
        let mut query: Vec<(String, String)> = vec![];
        if let Some(limit) = self.limit {
            query.push((String::from("limit"), limit.to_string()));
        }
        if let Some(offset) = self.offset {
            query.push((String::from("offset"), offset.to_string()));
        }
        if let Some(includefields) = &self.includefields {
            query.push((String::from("includefields"), includefields.to_string()));
        }
        for filter in self.filters.iter() {
            query.push((filter.0.to_string(), filter.1.clone()));
        }

        let mut request = self.client.client.get(&self.url).query(&query);

        // Basic authentication
        if self.client.username.is_some() {
            request = request.basic_auth(
                self.client.username.as_ref().unwrap().to_string(),
                self.client.password.as_ref(),
            );
        }
        // Bearer token
        else if self.client.bearer_token.is_some() {
            request = request.bearer_auth(self.client.bearer_token.as_ref().unwrap());
        }

        let text = self
            .client
            .client
            .get(&self.url)
            .query(&query)
            .send()
            .await
            .context(RequestFailedSnafu { url: &self.url })?
            .text()
            .await
            .context(DeserializationFailedSnafu {})?;
        println!("{}", text);

        Ok(request
            .send()
            .await
            .context(RequestFailedSnafu { url: &self.url })?
            .json::<Vec<DicomJson<InMemDicomObject>>>()
            .await
            .context(DeserializationFailedSnafu {})?
            .into_iter()
            .map(|dj| dj.into_inner())
            .collect())
    }

    pub fn with_limit(self: &mut Self, limit: u32) -> &mut Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(self: &mut Self, offset: u32) -> &mut Self {
        self.offset = Some(offset);
        self
    }

    pub fn with_includefields(self: &mut Self, includefields: Vec<Tag>) -> &mut Self {
        self.includefields = Some(
            includefields
                .iter()
                .map(|tag| tag.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );

        self
    }

    pub fn with_fuzzymatching(self: &mut Self, fuzzymatching: bool) -> &mut Self {
        self.fuzzymatching = Some(fuzzymatching);
        self
    }

    pub fn with_filter(self: &mut Self, tag: Tag, value: String) -> &mut Self {
        self.filters.push((tag, value));
        self
    }
}

impl DicomWebClient {
    pub fn query_studies(&self) -> QidoRequest {
        let url = format!("{}/studies", self.qido_url);

        QidoRequest::new(self.clone(), url)
    }

    pub fn query_series(&self) -> QidoRequest {
        let url = format!("{}/series", self.qido_url);

        QidoRequest::new(self.clone(), url)
    }

    pub fn query_series_in_study(&self, study_instance_uid: &str) -> QidoRequest {
        let url = format!("{}/studies/{}/series", self.qido_url, study_instance_uid);

        QidoRequest::new(self.clone(), url)
    }

    pub fn query_instances(&self) -> QidoRequest {
        let url = format!("{}/instances", self.qido_url);

        QidoRequest::new(self.clone(), url)
    }

    pub fn query_instances_in_series(
        &self,
        study_instance_uid: &str,
        series_instance_uid: &str,
    ) -> QidoRequest {
        let url = format!(
            "{}/studies/{}/series/{}/instances",
            self.qido_url, study_instance_uid, series_instance_uid
        );

        QidoRequest::new(self.clone(), url)
    }
}
