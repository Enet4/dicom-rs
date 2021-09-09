use dicom::object::{mem::InMemDicomObject, StandardDataDictionary};
use snafu::Snafu;
use snafu::ResultExt;
use ureq::Response;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::{self, BufReader};

use std::collections::HashMap;

pub mod utils;
pub mod decode;
pub use utils::{iter_json_array, download_multipart};
pub use decode::decode_response_item;

#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    UreqError { source: ureq::Error },
    BadResponse { status: u16 },
    BadJson,
    BadQuery { what: String },
    OpenFile { filename: PathBuf , source: std::io::Error },
    DecodeJson { source: std::io::Error },
    ExpandUser { source: std::io::Error, path: String },
    Retrieve { source: std::io::Error },
    Weired
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub fn qido_path(study: Option<&str>, serie: Option<&str>, instance: Option<&str>) -> Result<String> {
    match (study, serie, instance) {
        (Some(study), Some(serie), Some(instance)) => Ok(format!(
            "studies/{}/series/{}/instances/{}",
            study, serie, instance
        )),
        (Some(study), Some(serie), None) => Ok(format!(
            "studies/{}/series/{}/instances",
            study, serie
        )),
        (Some(study), None, None) => Ok(format!("studies/{}/series", study)),
        (None, None, None) => Ok("studies".to_string()),
        _ => Err(Error::BadQuery {
            what: "Missing study or serie ID".into(),
        }),
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct Qido {
    uri: String,
    arguments: HashMap<String, String>,
}


impl Qido {
    pub fn studies() -> Self {
        let path = qido_path(None, None, None).unwrap();
        Qido::new(&path)
    }
    pub fn series(study_id: &str) -> Self {
        let path = qido_path(Some(study_id), None, None).unwrap();
        Qido::new(&path)
    }
    pub fn instances(study_id: &str, serie_id: &str) -> Self {
        let path = qido_path(Some(study_id), Some(serie_id), None).unwrap();
        Qido::new(&path)
    }
    pub fn instance(study_id: &str, serie_id: &str, instance_id: &str) -> Self {
        let path = qido_path(Some(study_id), Some(serie_id), Some(instance_id)).unwrap();
        Qido::new(&path)
    }

    pub fn new(uri: &str) -> Self {
        Qido {
            uri: uri.into(),
            ..Default::default()
        }
    }

    pub fn add_argument(&mut self, name: &str, value: &str) -> &mut Self {
        self.arguments.insert(name.into(), value.into());
        self
    }

    pub fn build(&self, cli: &DicomSquare) -> ureq::Request {
        let mut req = cli.get_dicom_json(&self.uri);
        for (k,v) in self.arguments.iter() {
            req = req.query(k, v)
        }
        req
    }
}
type DicomResponse = InMemDicomObject<StandardDataDictionary>;
pub struct DicomSquare {
    pub uri: String,
    agent: ureq::Agent,
    local_storage: PathBuf,
}

impl DicomSquare {
    
    pub fn new(uri: &str, local_storage: &str, timeout: u64) -> Self {
        Self {
            uri: uri.into(),
            agent: ureq::AgentBuilder::new()
                .timeout_read(Duration::from_secs(timeout))
                .timeout_write(Duration::from_secs(timeout))
                .build(),
            local_storage: expanduser::expanduser(local_storage).unwrap()
        }
    }

    pub fn get_dicom_json(&self, path: &str) -> ureq::Request {
        self.agent
            .get(&self.endpoint(path))
            .set("Accept", "application/dicom+json, application/json, *")
    }
    pub fn post_dicom_json(&self, path: &str) -> ureq::Request {
        self.agent
            .post(&self.endpoint(path))
            .set("Accept", "application/dicom+json, application/json, *")
    }

    pub fn local_path(&self, study: Option<&str>, serie: Option<&str>, instance: Option<&str>) -> PathBuf {
        let path = match (study, serie, instance) {
                (Some(study), Some(serie), Some(instance)) => Ok(format!(
                    "studies/{}/series/{}/instances/{}.dcm", study, serie, instance
                )),
                (Some(study), Some(serie), None) => Ok(format!(
                    "studies/{}/series/{}", study, serie
                )),
                (Some(study), None, None) => Ok(format!(
                    "studies/{}", study
                )),
                (None, None, None) => Ok("studies".to_string()),
                _ => { Err(Error::BadQuery { what: "Missing study or serie ID".into() })}
            }.unwrap();
        let mut full_path = self.local_storage.clone();
        full_path.push(path.as_str());
        full_path
    }

    pub fn endpoint(&self, path: &str) -> String {
        if path.is_empty() {
            format!("{}/", self.uri)
        } else if !path.starts_with('/') {
            format!("{}/{}", self.uri, path)
        } else {
            format!("{}{}", self.uri, path)
        }
    }
    pub fn url(
        &self,
        what: &str,
        study: Option<&str>,
        serie: Option<&str>,
        instance: Option<&str>,
    ) -> Result<String> {
        let what = match what {
            "/metadata" | "/file" | "" => Ok(what),
            other => Err(Error::BadQuery { what: other.into() }),
        }?;
        match (study, serie, instance) {
            (Some(study), Some(serie), Some(instance)) => Ok(format!(
                "{}/studies/{}/series/{}/instances/{}{}",
                self.uri, study, serie, instance, what
            )),
            (Some(study), Some(serie), None) => Ok(format!(
                "{}/studies/{}/series/{}{}",
                self.uri, study, serie, what
            )),
            (Some(study), None, None) => Ok(format!("{}/studies/{}{}", self.uri, study, what)),
            (None, None, None) => Ok(format!("{}/studies", self.uri)),
            _ => Err(Error::BadQuery {
                what: "Missing study or serie ID".into(),
            }),
        }
    }
    pub fn search_studies(&self) -> ureq::Request {
        let url = format!("{}/studies", self.uri);
        self.make_query(&url, "application/dicom+json")
    }
    pub fn search_series(&self) -> ureq::Request {
        let url = format!("{}/series", self.uri);
        self.make_query(&url, "application/dicom+json")
    }
    pub fn search_instances(&self) -> ureq::Request {
        let url = format!("{}/instances", self.uri);
        self.make_query(&url, "application/dicom+json")
    }

    fn make_query(&self, url: &str, accepting: &str) -> ureq::Request {
        self.agent.get(url).set("Accept", accepting)

        // "application/dicom+json")
    }
    pub fn metadata(
        &self,
        study: Option<&str>,
        serie: Option<&str>,
        instance: Option<&str>,
    ) -> Result<ureq::Request> {
        let url = self.url("/metadata", study, serie, instance)?;
        Ok(self.make_query(&url, "application/dicom+json"))
    }
    pub fn retrieve(
        &self,
        study: Option<&str>,
        serie: Option<&str>,
        instance: Option<&str>,
    ) -> Result<ureq::Request> {
        let url = self.url("", study, serie, instance)?;
        Ok(self.make_query(&url, r#"multipart/related; type="application/dicom""#))
    }

    pub fn fetch_json_array<T: DeserializeOwned>(
        &self,
        resp: ureq::Response,
    ) -> Result<impl Iterator<Item = Result<T, io::Error>>> {
        if resp.status() != 200 {
            Err(Error::BadResponse {
                status: resp.status(),
            })
        } else {
            let reader = resp.into_reader();
            Ok(iter_json_array(reader))
        }
    }
    pub fn fetch_json_object<T: DeserializeOwned>(
        &self,
        resp: ureq::Response,
    ) -> Result<T> {
        resp.into_json().context(DecodeJson)
    }

    pub fn qido(&self, qido: &Qido) -> Result<Response> {
        let req = qido.build(self);
        req.call().context(UreqError)
    }

    pub fn store(&self, path: &Path) -> Result<Response> {
        if let Some(raw) = path.to_str() {
            let path: PathBuf = expanduser::expanduser(raw).context(ExpandUser { path: raw })?;
            let file = File::open(&path).with_context(|| OpenFile { filename: path.to_owned()})?;
            let sz = file.metadata().unwrap().len();
            let reader =
                BufReader::new(File::open(&path).with_context(|| OpenFile { filename: path })?);
            let content_type = r#"multipart/related; type="application/dicom"; boundary=--xoxoxoxo-xixixixix-xaxaxaxax"#;
            let req = self.post_dicom_json("/studies")
                .set("Content-Type", content_type)
                .set("Content-Length", sz.to_string().as_str());
            req.send(reader).context(UreqError)
        } else {
            Err(Error::Weired)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{DicomResponse, DicomSquare, Error, Qido, decode_response_item};
    use serde_json::{json, Value};
    #[test]
    fn qido_tag_name() {
        let endpoint = "/studies";
        let mut qido = Qido::new(endpoint);
        let qido = qido
            .add_argument("fuzzymatching", "false")
            .add_argument("limit", "101")
            .add_argument("PatientID", "046"); // 00100020

        // assert_eq!(format!("{:?}", qido),
        //  "Qido { uri: \"/studies\", arguments: {\"fuzzymatching\": \"true\", \"00100020\": \"046\", \"limit\": \"101\"} }");
        let cli = DicomSquare::new("http://localhost:8042/dicom-web", "", 5);
        // let req = qido.build(&cli);

        match cli.qido(qido) {
            Ok(resp) => {
                assert_eq!(resp.status(), 200);
                let mut check: Vec<DicomResponse> = vec![];
                for obj in cli.fetch_json_array(resp).unwrap() {
                    let dicom = decode_response_item(&obj.unwrap());
                    check.push(dicom);
                }
                assert_eq!(check.len(), 1);
            }
            Err(Error::UreqError{ source }) => {
                match source {
                    ureq::Error::Status(status, resp) => {
                        eprintln!("error: {}: {:#?}", status, resp);
                        assert_eq!(status, 400);
                        let explain: Value = resp.into_json().unwrap();
                        assert_eq!(explain, json!({}));
                    },
                    ureq::Error::Transport(transport) => {
                        eprintln!("transport error: {:#?}", transport);
                        assert_eq!(true, false);
                    }
                }
            },
            Err(err) => {
                panic!("unexpected error: {:#?}", err);
            }
        };
    }
    #[test]
    fn qido_tag_value() {
        let endpoint = "/studies";
        let mut qido = Qido::new(endpoint);
        let qido = qido
            .add_argument("fuzzymatching", "false")
            .add_argument("limit", "101")
            .add_argument("00100020", "046"); // PatientID

        // assert_eq!(format!("{:?}", qido),
        //  "Qido { uri: \"/studies\", arguments: {\"fuzzymatching\": \"true\", \"00100020\": \"046\", \"limit\": \"101\"} }");
        let cli = DicomSquare::new("http://localhost:8042/dicom-web", "", 5);
        // let req = qido.build(&cli);

        match cli.qido(qido) {
            Ok(resp) => {
                assert_eq!(resp.status(), 200);
                let mut check: Vec<DicomResponse> = vec![];
                for obj in cli.fetch_json_array(resp).unwrap() {
                    let dicom = decode_response_item(&obj.unwrap());
                    check.push(dicom);
                }
                assert_eq!(check.len(), 1);
            }
            Err(Error::UreqError{ source }) => {
                match source {
                    ureq::Error::Status(status, resp) => {
                        eprintln!("error: {}: {:#?}", status, resp);
                        assert_eq!(status, 400);
                        let explain: Value = resp.into_json().unwrap();
                        assert_eq!(explain, json!({}));
                    },
                    ureq::Error::Transport(transport) => {
                        eprintln!("transport error: {:#?}", transport);
                        assert_eq!(true, false);
                    }
                }
            },
            Err(err) => {
                panic!("unexpected error: {:#?}", err);
            }
        };
    }
    #[test]
    fn query_studies() {
        let qido = Qido::studies();
        let cli = DicomSquare::new("http://localhost:8042/dicom-web","", 5);
        match cli.qido(&qido) {
            Ok(resp) => {
                assert_eq!(resp.status(), 200);
                let mut check: Vec<DicomResponse> = vec![];
                for obj in cli.fetch_json_array(resp).unwrap() {
                    let dicom = decode_response_item(&obj.unwrap());
                    check.push(dicom);
                }
                assert_eq!(check.len(), 2);
            },
            Err(_) => todo!(),
        }
    }
    #[test]
    fn query_study_series() {
        let qido = Qido::series("1.3.6.1.4.1.14301.77.4.14093378.1");
        let cli = DicomSquare::new("http://localhost:8042/dicom-web", "", 5);
        match cli.qido(&qido) {
            Ok(resp) => {
                assert_eq!(resp.status(), 200);
                let mut check: Vec<DicomResponse> = vec![];
                for obj in cli.fetch_json_array(resp).unwrap() {
                    let dicom = decode_response_item(&obj.unwrap());
                    check.push(dicom);
                }
                assert_eq!(check.len(), 2);
            },
            Err(Error::UreqError{ source }) => {
                match source {
                    ureq::Error::Status(status, resp) => {
                        eprintln!("error: {}: {:#?}", status, resp);
                        assert_eq!(status, 400);
                        let explain: Value = resp.into_json().unwrap();
                        assert_eq!(explain, json!({}));
                    },
                    ureq::Error::Transport(transport) => {
                        eprintln!("transport error: {:#?}", transport);
                        assert_eq!(true, false);
                    }
                }
            },
            Err(err) => {
                panic!("unexpected error: {:#?}", err);
            }
        }
    }
    #[test]
    fn query_serie_instances() {
        let qido = Qido::instances(
            "1.3.6.1.4.1.14301.77.4.14093378.1",
            "1.3.12.2.1107.5.2.19.45188.2019121218553837036261998.0.0.0"
        );
        let cli = DicomSquare::new("http://localhost:8042/dicom-web", "",5);
        match cli.qido(&qido) {
            Ok(resp) => {
                assert_eq!(resp.status(), 200);
                let mut check: Vec<DicomResponse> = vec![];
                for obj in cli.fetch_json_array(resp).unwrap() {
                    let dicom = decode_response_item(&obj.unwrap());
                    check.push(dicom);
                }
                assert_eq!(check.len(), 192);
            },
            Err(Error::UreqError{ source }) => {
                match source {
                    ureq::Error::Status(status, resp) => {
                        eprintln!("error: {}: {:#?}", status, resp);
                        assert_eq!(status, 400);
                        let explain: Value = resp.into_json().unwrap();
                        assert_eq!(explain, json!({}));
                    },
                    ureq::Error::Transport(transport) => {
                        eprintln!("transport error: {:#?}", transport);
                        assert_eq!(true, false);
                    }
                }
            },
            Err(err) => {
                panic!("unexpected error: {:#?}", err);
            }
        }
    }
    #[test]
    fn store() {
        let mut path = PathBuf::from(&expanduser::expanduser("~/JDD/22").unwrap());

        path.push("1.3.6.1.4.1.14301.77.4.14093378.1");
        path.push("1.3.12.2.1107.5.2.19.45188.2019121218553837036261998.0.0.0");
        path.push("1.3.12.2.1107.5.2.19.45188.2019121219011265545063168.dcm");
    
        let cli = DicomSquare::new("http://localhost:8042/dicom-web", "", 5);
        match cli.store(&path) {
            Ok(resp) => {
                assert_eq!(resp.status(), 200);
                let obj = cli.fetch_json_object(resp).unwrap();
                let _dicom = decode_response_item(&obj);
                //assert_eq!(check.len(), 192);
            },
            Err(Error::UreqError{ source }) => {
                match source {
                    ureq::Error::Status(status, resp) => {
                        eprintln!("error: {}: {:#?}", status, resp);
                        assert_eq!(status, 415);
                        let explain: Value = resp.into_json().unwrap();
                        assert_eq!(explain, json!({}));
                    },
                    ureq::Error::Transport(transport) => {
                        eprintln!("transport error: {:#?}", transport);
                        assert_eq!(true, false);
                    }
                }
            },
            Err(err) => {
                panic!("unexpected error: {:#?}", err);
            }
        }
    }
}
