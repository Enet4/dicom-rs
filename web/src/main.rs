
use std::{io::BufReader, path::PathBuf};
use dicom_dump::dump_object;
use dicom_web::{DicomSquare, download_multipart, decode_response_item};
use serde_json::json;
use structopt::StructOpt;
use dicom_web::Result;
use serde_json::Value;
#[derive(Debug, StructOpt)]
struct App {
	#[structopt(short="u", long="url", default_value="http://localhost:8042/dicom-web")]
	url: String,
	#[structopt(short="t", long="timeout", default_value="5")]
	timeout: u64,
	#[structopt(short="v", long="verbose")]
	verbose: bool,
	#[structopt(subcommand)]
	cmd: Command
}

#[derive(Debug, StructOpt)]
enum Command {
	List {
		what: String,
		#[structopt(short="k", long="key")]
		key: Vec<String>,
		#[structopt(short="l", long="limit")]
		limit: Option<u64>,
		#[structopt(short="o", long="offset")]
		offset: Option<u64>
	},
	Fetch {
		kind: String,
		#[structopt(long="study")]
		study: Option<String>,
		#[structopt(long="serie")]
		serie: Option<String>,
		#[structopt(long="instance")]
		instance: Option<String>,
	},
	Store {
		dicoms: Vec<PathBuf>
	}
}

fn main() -> Result<()> {
	let App {
		url,
		timeout,
		verbose,
		cmd
	} = App::from_args();
	let cli = DicomSquare::new(&url,".", timeout);
	match cmd {
		Command::List { 
			what ,
			key,
			limit,
			offset,
		} => {
		
			let mut req = match what.as_str() {
				"studies" => { cli.search_studies() },
				"series" => { cli.search_series() },
				"instances" => { cli.search_instances() },
				other => {
					eprintln!("unknown resource type: {}", other);
					std::process::exit(1);
				}
			};

			let mut query = json!({});
			for item in key {
				let vec = item.split('=').collect::<Vec<&str>>();
				query[vec[0]] = json!(vec[1]);
			}
			// if include.len() != 0 {
			//     query["includefield"] = json!(include);
			// }
			if let Some(limit) = limit {
				query["limit"] = json!(limit);
			}
			if let Some(offset) = offset {
				query["offset"] = json!(offset);
			}

			if let Value::Object(o) = query {
				for (k, v) in o {
					if let Some(v) = v.as_str() {
						req = req.query(&k, v)
					} else {
						println!("warning: no value");
					}
				}
			}
			if verbose {
				eprintln!("{:#?}", req);
			}
			let resp = req.call().unwrap();
			for obj in cli.fetch_json_array(resp)? {
				let obj: Value = obj.unwrap();
				let dicom = decode_response_item(&obj);
				dump_object(&dicom, false).unwrap();
			}
		},
		Command::Fetch {
			study,
			serie,
			instance,
			kind: part,
		 } => {

			let study = study.as_deref();
			let serie = serie.as_deref();
			let instance = instance.as_deref();
			match part.as_str() {
				"metadata" => {
					let req = cli.metadata(study, serie, instance).unwrap();
					if verbose { eprintln!("{:#?}", req); }
					let resp = req.call().unwrap();
					for obj in cli.fetch_json_array(resp)? {
						let obj: Value = obj.unwrap();
						let dicom = decode_response_item(&obj);
						dump_object(&dicom, false).unwrap();
					}
				},
				"content" => {
					let req = cli.retrieve(study, serie, instance).unwrap();
					if verbose { eprintln!("{:#?}", req); }
					let resp = req.call().unwrap();
					let path = cli.local_path(study, serie, instance);

					if let Some(parent) = path.parent() {
						let _ = std::fs::create_dir_all(parent);
					}
					let file= std::fs::File::create(path).unwrap();
					let mut reader = BufReader::new(resp.into_reader());
					download_multipart(&mut reader, file, verbose).unwrap();

				},
				other => { eprintln!("todo! Retrieve {}", other); std::process::exit(1); }
			}
		},
		Command::Store { dicoms } => {
			dicoms.iter().for_each(|path| {
				let resp = cli.store(&path).unwrap();
				// eprintln!("resp: {:#?}", resp);
				let j: Value = resp.into_json().unwrap();
				let dicom = decode_response_item(&j);
				dump_object(&dicom, false).unwrap();
				// eprintln!("{:#?}", j);
				// or obj in cli.fetch_json_array(resp).unwrap() {
				//	let obj: Value = obj.unwrap();
					//let dicom = decode_response_item(&obj);
					//dump_object(&dicom).unwrap();
				// }
			});
		}
	};
	Ok(())
}
