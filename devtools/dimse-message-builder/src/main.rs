use clap::Parser;
use eyre::{Context, ContextCompat, Result, eyre};
use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::quote;
use sxd_document::{Package, parser};
use sxd_xpath::Factory;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Generate DICOM command structs from the DICOM standard
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output file path relative to workspace root
    #[arg(short, long, default_value = "ul/src/pdu/generated.rs")]
    output: String,
}

// Column labels from the DICOM standard tables
const DESCRIPTION_COLUMN: &str = "Description of Field";
const REQ_COLUMN: &str = "Req/Ind";
const RESP_COLUMN: &str = "Rsp/Conf";
const CANCEL_COLUMN: &str = "CnclReq/CnclInd";
const VR_COLUMN: &str = "VR";

// Struct suffixes
const REQ_SUFFIX: &str = "Rq";
const RESP_SUFFIX: &str = "Rsp";
const CANCEL_SUFFIX: &str = "Cncl";

// VR to Rust type mapping
fn get_vr_type(vr: &str) -> Result<&'static str> {
    match vr {
        "US" => Ok("u16"),
        "UI" => Ok("&'a str"),
        "AE" => Ok("&'a str"),
        _ => Err(eyre!("No type found for VR {}", vr)),
    }
}

// Table groups for each type of message
const TABLE_GROUPS: &[(&str, &[&str])] = &[
    ("C_Store", &[
        "table_9.1-1",  // C-STORE overview
        "table_9.3-1",  // C-STORE-RQ tag definitions
        "table_9.3-2",  // C-STORE-RSP tag definitions
    ]),
    ("C_Find", &[
        "table_9.1-2",  // C-FIND overview
        "table_9.3-3",  // C-FIND-RQ tag definitions
        "table_9.3-4",  // C-FIND-RSP tag definitions
        "table_9.3-5",  // C-CANCEL-FIND-RQ tag definitions
    ]),
    ("C_Get", &[
        "table_9.1-3",  // C-GET overview
        "table_9.3-6",  // C-GET-RSP tag definitions
        "table_9.3-7",  // C-GET-RSP tag definitions
        "table_9.3-8",  // C-CANCEL-GET-RQ tag definitions
    ]),
    ("C_Move", &[
        "table_9.1-4",  // C-MOVE overview
        "table_9.3-9",  // C-MOVE-RSP tag definitions
        "table_9.3-10", // C-MOVE-RSP tag definitions
        "table_9.3-11", // C-CANCEL-MOVE-RQ tag definitions
    ]),
    ("C_Echo", &[
        "table_9.1-5", // C-ECHO overview
        "table_9.3-12", // C-ECHO-RQ tag definitions
        "table_9.3-13", // C-ECHO-RSP tag definitions
    ]),
];

#[derive(Debug, Clone)]
struct TableRow {
    param: String,
    data: HashMap<String, String>,
}

#[derive(Debug)]
struct ParsedTable {
    headers: Vec<String>,
    rows: Vec<TableRow>,
}

fn fetch_dicom_xml() -> Result<(sxd_document::Package, sxd_xpath::Context<'static>)> {
    let url = "https://dicom.nema.org/medical/dicom/current/source/docbook/part07/part07.xml";
    let response = reqwest::blocking::get(url)
        .context("Failed to fetch DICOM standard XML")?;
    let text = response.text()
        .context("Failed to read response text")?;
    let package = parser::parse(&text).map_err(|e| eyre!("Failed to parse XML: {}", e))?;
    let context = {
        let mut ctx = sxd_xpath::Context::new();
        ctx.set_namespace("book", "http://docbook.org/ns/docbook");
        ctx.set_namespace("xlink", "http://www.w3.org/1999/xlink");
        ctx.set_namespace("xml", "http://www.w3.org/XML/1998/namespace");
        ctx
    };
    Ok((package, context))
}

fn parse_table(xml: &Package, context: &sxd_xpath::Context<'static>, table_id: &str) -> Result<ParsedTable> {
    let document = xml.as_document();
    let factory = Factory::new();

    // Find the table by ID
    let xpath_expr = factory.build(&format!("//book:table[@xml:id='{table_id}']"))
        .context("Could not compile XPath to table")?
        .context("No path was compiled")?;
    let table_nodes = xpath_expr.evaluate(context, document.root())
        .map_err(|e| eyre!("XPath evaluation failed: {}", e))?;

    let table_node = match &table_nodes {
        sxd_xpath::Value::Nodeset(nodes) => {
            nodes.iter().collect::<Vec<_>>()
        },
        _ => return Err(eyre!("Table {} not found", table_id)),
    };
    let table_node = table_node.first()
        .ok_or_else(|| eyre!("Table {} not found", table_id))?;

    // Parse headers from th elements
    let header_xpath = factory.build("./book:thead/book:tr/book:td | ./book:thead/book:tr/book:th")
        .context("Could not compile XPath to table")?
        .context("No path was compiled")?;
    let header_nodes = header_xpath.evaluate(context, *table_node)
        .map_err(|e| eyre!("XPath evaluation failed: {}", e))?;

    let mut headers = Vec::new();
    if let sxd_xpath::Value::Nodeset(nodes) = &header_nodes {
        for node in nodes.document_order() {
            let text = node.string_value().trim().to_string();
            if !text.is_empty() {
                headers.push(text);
            }
        }
    }

    // If no headers found in th elements, try first row td elements
    if headers.is_empty() {
        let first_row_xpath = factory.build("./book:tbody/book:tr[1]/book:td")
            .context("Could not compile XPath to table")?
            .context("No path was compiled")?;
        let first_row_nodes = first_row_xpath.evaluate(context, *table_node)
            .map_err(|e| eyre!("XPath evaluation failed: {}", e))?;

        if let sxd_xpath::Value::Nodeset(nodes) = &first_row_nodes {
            for node in nodes.document_order() {
                let text = node.string_value().trim().to_string();
                headers.push(text);
            }
        }
    }

    // Parse data rows
    let row_xpath = factory.build("./book:tbody/book:tr")
        .context("Could not compile XPath to table")?
        .context("No path was compiled")?;
    let row_nodes = row_xpath.evaluate(context, *table_node)
        .map_err(|e| eyre!("XPath evaluation failed: {}", e))?;

    let mut rows = Vec::new();
    if let sxd_xpath::Value::Nodeset(nodes) = &row_nodes {
        for row_node in nodes {
            // For each row, get the cells
            let cell_xpath = factory.build("./book:td")
                .context("Could not compile XPath to table")?
                .context("No path was compiled")?;
            let cell_nodes = cell_xpath.evaluate(context, row_node)
                .map_err(|e| eyre!("XPath evaluation failed: {}", e))?;

            let mut cells = Vec::new();
            if let sxd_xpath::Value::Nodeset(cell_node_set) = &cell_nodes {
                for cell_node in cell_node_set.document_order() {
                    let text = cell_node.string_value().trim().to_string();
                    cells.push(text);
                }
            }

            if cells.is_empty() {
                continue;
            }

            let param = cells.first().unwrap_or(&String::new()).clone();
            if param.is_empty() {
                continue;
            }

            let mut data = HashMap::new();
            for (i, header) in headers.iter().skip(1).enumerate() {
                if let Some(cell_value) = cells.get(i + 1) {
                    data.insert(header.clone(), cell_value.clone());
                }
            }

            // Set param as first column header
            let param_header = headers.first().unwrap_or(&"param".to_string()).clone();
            data.insert(param_header, param.clone());

            rows.push(TableRow { param, data });
        }
    }

    Ok(ParsedTable { headers, rows })
}

fn merge_tables(overview_table: ParsedTable, tag_def_tables: Vec<ParsedTable>) -> ParsedTable {
    let mut merged_rows = overview_table.rows;
    let mut all_headers = overview_table.headers.clone();

    // Add all tag definition rows
    for tag_table in tag_def_tables {
        for row in tag_table.rows {
            // Check if this parameter already exists
            if let Some(existing_row) = merged_rows.iter_mut().find(|r| r.param == row.param) {
                // Merge data from tag definition
                for (key, value) in row.data {
                    existing_row.data.insert(key.clone(), value);
                    if !all_headers.contains(&key) {
                        all_headers.push(key);
                    }
                }
            } else {
                merged_rows.push(row);
            }
        }

        // Add headers from tag tables
        for header in tag_table.headers {
            if !all_headers.contains(&header) {
                all_headers.push(header);
            }
        }
    }

    ParsedTable {
        headers: all_headers,
        rows: merged_rows,
    }
}

fn generate_struct_field(param: &str, need: &str, vr: &str, description: &str) -> Result<Option<TokenStream>> {
    let field_name = param.to_snake_case();

    // Exclusions
    if param == "Data Set" || param == "Identifier" {
        return Ok(None);
    }
    if need == "-" {
        return Ok(None);
    }

    // Special handling for Priority
    if param == "Priority" {
        return Ok(Some(quote! {
            /// Priority for the request
            #[builder(default = Priority::Medium)]
            pub priority: Priority
        }));
    }

    let rust_type = get_vr_type(vr)?;
    let field_ident = syn::Ident::new(&field_name, proc_macro2::Span::call_site());

    let type_tokens: TokenStream = if need.starts_with('M') {
        rust_type.parse().unwrap()
    } else {
        format!("Option<{rust_type}>").parse().unwrap()
    };

    Ok(Some(quote! {
        #[doc = #description]
        pub #field_ident: #type_tokens
    }))
}

fn generate_dataset_field(param: &str, need: &str, vr: &str) -> Result<(Vec<TokenStream>, Vec<String>)> {
    let mut impl_fields = Vec::new();
    let mut markers = Vec::new();

    // Special handling for Data Set and Identifier
    if param == "Data Set" || param == "Identifier" {
        if need.starts_with('M') {
            impl_fields.push(quote! {
                DE::new(tags::COMMAND_DATA_SET_TYPE, VR::US, value!(0x0001))
            });
            markers.push("DatasetRequiredCommand".to_string());
        } else if need.starts_with('U') || need.starts_with('C') {
            markers.push("DatasetConditionalCommand".to_string());
            markers.push("DatasetRequiredCommand".to_string());
            impl_fields.push(quote! {
                DE::new(tags::COMMAND_DATA_SET_TYPE, VR::US, value!(0x0101))
            });
        } else {
            markers.push("DatasetForbiddenCommand".to_string());
            impl_fields.push(quote! {
                DE::new(tags::COMMAND_DATA_SET_TYPE, VR::US, value!(0x0101))
            });
        }
        return Ok((impl_fields, markers));
    }

    if need == "-" {
        return Ok((impl_fields, markers));
    }

    let field_name = param.to_snake_case();
    let field_ident = syn::Ident::new(&field_name, proc_macro2::Span::call_site());
    let tag_ident = syn::Ident::new(&field_name.to_uppercase(), proc_macro2::Span::call_site());
    let vr_ident = syn::Ident::new(vr, proc_macro2::Span::call_site());

    // Special handling for priority which needs casting
    if field_name == "priority" {
        impl_fields.push(quote! {
            DE::new(tags::#tag_ident, VR::#vr_ident, value!(self.#field_ident as u16))
        });
    } else {
        impl_fields.push(quote! {
            DE::new(tags::#tag_ident, VR::#vr_ident, value!(self.#field_ident))
        });
    }

    Ok((impl_fields, markers))
}

fn generate_command_struct(
    prefix: &str,
    column_name: &str,
    suffix: &str,
    merged_table: &ParsedTable,
) -> Result<TokenStream> {
    let struct_name_str = if suffix == "Cncl" {
        format!("{}Cncl", prefix.replace('_', ""))
    } else {
        format!("{}{}", prefix.replace('_', ""), suffix)
    };
    let struct_name = syn::Ident::new(&struct_name_str, proc_macro2::Span::call_site());

    let command_field_name = if suffix == "Cncl" {
        "C_CANCEL_RQ".to_string()
    } else {
        format!("{}_{}_{}", prefix.to_uppercase(), suffix.to_uppercase(), "")
            .trim_end_matches('_')
            .to_string()
    };
    let command_field_ident = syn::Ident::new(&command_field_name, proc_macro2::Span::call_site());

    let mut struct_fields = Vec::new();
    let mut impl_fields = Vec::new();
    let mut all_markers = Vec::new();

    for row in &merged_table.rows {
        let param = &row.param;

        // Handle sub-operations naming
        let param = if param.contains("Sub-operations") {
            param.replace("Sub-operations", "Suboperations")
        } else {
            param.clone()
        };

        let need = row.data.get(column_name).unwrap_or(&"-".to_string()).clone();
        let vr = row.data.get(VR_COLUMN).unwrap_or(&"".to_string()).clone();
        let description = row.data.get(DESCRIPTION_COLUMN)
            .unwrap_or(&param)
            .replace('\n', " ")
            .replace('\r', "");

        // Generate struct field
        if let Some(field) = generate_struct_field(&param, &need, &vr, &description)? {
            struct_fields.push(field);
        }

        // Generate dataset implementation
        let (dataset_fields, markers) = generate_dataset_field(&param, &need, &vr)?;
        impl_fields.extend(dataset_fields);
        all_markers.extend(markers);
    }

    // If no marker traits were added, default to DatasetForbiddenCommand
    if all_markers.is_empty() {
        all_markers.push("DatasetForbiddenCommand".to_string());
    }

    let marker_impls: Vec<TokenStream> = all_markers
        .into_iter()
        .map(|marker| {
            let marker_ident = syn::Ident::new(&marker, proc_macro2::Span::call_site());
            if suffix == "Cncl" {
                quote! { impl #marker_ident for #struct_name {} }
            } else {
                quote! { impl<'a> #marker_ident for #struct_name<'a> {} }
            }
        })
        .collect();

    let lifetime = if suffix == "Cncl" {
        quote! {}
    } else {
        quote! { <'a> }
    };

    Ok(quote! {

        #[derive(Builder, Debug)]
        pub struct #struct_name #lifetime {
            #(#struct_fields,)*
        }

        impl #lifetime Command for #struct_name #lifetime {
            fn command_field(&self) -> u16 {
                CommandField::#command_field_ident as u16
            }

            fn dataset(&self) -> InMemDicomObject {
                InMemDicomObject::from_element_iter(vec![
                    DE::new(tags::COMMAND_FIELD, VR::US, value!(self.command_field())),
                    #(#impl_fields,)*
                ])
            }
        }

        #(#marker_impls)*

    })
}

fn generate_all_structs() -> Result<syn::File> {
    println!("Fetching DICOM standard XML...");
    let (package, context) = fetch_dicom_xml()?;

    let mut generated_structs = Vec::new();

    for (prefix, table_ids) in TABLE_GROUPS {
        println!("Processing message group: {prefix}");

        let overview_id = table_ids[0];
        let tag_def_ids = &table_ids[1..];

        // Parse overview table
        let overview_table = parse_table(&package, &context, overview_id)?;

        // Parse tag definition tables
        let mut tag_def_tables = Vec::new();
        for &tag_id in tag_def_ids {
            match parse_table(&package, &context, tag_id) {
                Ok(table) => tag_def_tables.push(table),
                Err(e) => {
                    println!("Warning: Could not parse table {tag_id}: {e}");
                    continue;
                }
            }
        }

        // Merge tables
        let merged_table = merge_tables(overview_table, tag_def_tables);

        // Generate structs for each message type
        for (column_name, suffix) in [
            (REQ_COLUMN, REQ_SUFFIX),
            (RESP_COLUMN, RESP_SUFFIX),
            (CANCEL_COLUMN, CANCEL_SUFFIX),
        ] {
            // Check if this message type exists for this group
            let has_column = merged_table.headers.contains(&column_name.to_string());
            if !has_column {
                continue;
            }

            match generate_command_struct(prefix, column_name, suffix, &merged_table) {
                Ok(struct_tokens) => {
                    generated_structs.push(struct_tokens);
                    let struct_name = if suffix == "Cncl" {
                        format!("{}Cncl", prefix.replace('_', ""))
                    } else {
                        format!("{}{}", prefix.replace('_', ""), suffix)
                    };
                    println!("Generated struct {struct_name}");
                }
                Err(e) => {
                    println!("Error generating struct for {prefix}{suffix}: {e}",);
                }
            }
        }
    }

    let file_content = quote! {
        //! Auto-generated DICOM command structs
        //! Do not hand edit, see `devtools/dimse-message-builder` for details
        use dicom_core::{DataElement as DE, VR, dicom_value as value};
        use dicom_object::InMemDicomObject;
        use bon::Builder;
        use dicom_dictionary_std::tags;
        use crate::pdu::commands::{
            CommandField, Priority, Command, DatasetRequiredCommand,
            DatasetConditionalCommand, DatasetForbiddenCommand
        };

        #(#generated_structs)*
    };

    Ok(syn::parse2(file_content)?)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let output_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().expect("No parent dir found")
        .parent().expect("No parent dir found")
        .join(args.output);

    let syntax_tree = generate_all_structs()?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Format the syntax tree using prettyplease
    // Could also change to rustfmt here as some of the formatting
    // is bizarre
    let formatted_code = prettyplease::unparse(&syntax_tree);
    fs::write(&output_path, formatted_code)?;

    println!("Generated Rust structs written to {}", output_path.display());
    Ok(())
}