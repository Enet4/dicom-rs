import requests
from bs4 import BeautifulSoup
import pandas as pd
import textwrap
from pathlib import Path

# Configuration
DESCRIPTION_COLUMN = "Description of Field"
REQ_COLUMN="Req/Ind"
RESP_COLUMN="Rsp/Conf"
CANCEL_COLUMN="CnclReq/CnclInd"
VR_COLUMN="VR"
REQ_SUFFIX="Rq"
RESP_SUFFIX="Rsp"
CANCEL_SUFFIX="Cncl"

VR_MAP = {
    "US": "u16",
    "UI": "&'a str",
    "AE": "&'a str"
}

table_groups = [
    ("C_Store", (
        "table_9.1-1",  # C-STORE overview
        "table_9.3-1",  # C-STORE-RQ tag definitions
        "table_9.3-2",  # C-STORE-RSP tag definitions
    )),
    ("C_Find", (
        "table_9.1-2",  # C-FIND overview
        "table_9.3-3",  # C-FIND-RQ tag definitions
        "table_9.3-4",  # C-FIND-RSP tag definitions
        "table_9.3-5",  # C-CANCEL-FIND-RQ tag definitions
    )),
    ("C_Get", (
        "table_9.1-3",  # C-GET overview
        "table_9.3-6",  # C-GET-RSP tag definitions
        "table_9.3-7",  # C-GET-RSP tag definitions
        "table_9.3-8",  # C-CANCEL-GET-RQ tag definitions
    )),
    ("C_Move", (
        "table_9.1-4",  # C-MOVE overview
        "table_9.3-9",  # C-MOVE-RSP tag definitions
        "table_9.3-10", # C-MOVE-RSP tag definitions
        "table_9.3-11", # C-CANCEL-MOVE-RQ tag definitions
    )),
    ("C_Echo", (
        "table_9.1-5", # C-ECHO overview
        "table_9.3-12", # C-ECHO-RQ tag definitions
        "table_9.3-13", # C-ECHO-RSP tag definitions
    ))
]

def get_tables():
    url = "https://dicom.nema.org/medical/dicom/current/output/html/part07.html"
    response = requests.get(url)
    soup = BeautifulSoup(response.text, "html.parser")
    table_list = soup.find_all("div", {"class", "table"})
    return table_list

def find_table(table_list, table_id):
    tables = {}
    for table in table_list:
        table_def = table.contents[1]
        if table_def.has_attr("id") and table_def['id'] == table_id:
            return table


def df_for_table(table):
    data = []
    for row in table.find_all("tr"):
        cols = row.find_all("td")
        cols = [element.text.strip() for element in cols]
        data.append([element for element in cols])

    headers = []
    for row in table.find_all("th"):
        header = [element.text.strip() for element in row.find_all("strong")][0]
        headers.append(header)
    if headers is None or headers == []:
        df = pd.DataFrame(data[1:], columns=data[0])
    else:
        df = pd.DataFrame(data, columns=headers)
    df.columns = ["param", *df.columns[1:]]
    df = df.set_index("param")
    df = df.dropna()
    return df

def get_merged_df(table_list, overview_id, *tag_def_ids):
    overview = df_for_table(find_table(table_list, overview_id))
    # Get the definitions for tags for each specific command
    tag_defs = [df_for_table(find_table(table_list, table)) for table in tag_def_ids]
    # Add the values of all possible tags together
    tag_def = pd.concat(tag_defs).drop_duplicates()
    tag_def = tag_def.dropna()
    # Join now the overview table with the tags, dropping duplicate tag names
    full = overview.join(tag_def).reset_index().drop_duplicates(subset=['param'])
    return full

def gen():
    table_list = get_tables()
    generated = ""

    def add_field_to_struct(struct, param, need, vr, description):
        # Convert parameter name to Rust field name (snake_case)
        field_name = param.lower().replace(' ', '_').replace('-', '_')
        # Exclusions: Don't add dataset/identifier to struct
        # and priority has a fixed field.
        if param in ["Data Set", "Identifier"]:
            return
        if param == 'Priority':
            struct.append(
                "/// Priority for the request\n"
                "#[builder(default = Priority::Medium)]\n"
                "priority: Priority"
            )
            return
        if vr not in VR_MAP:
            raise RuntimeError(f"No type found for VR {vr}: row {row}")
        value_type = VR_MAP.get(vr)
        if need[0] == 'M':
            type_ = f"{value_type}"
        else:
            type_ = f"Option<{value_type}>"
        struct.append(f"/// {description}")
        struct.append(f"pub {field_name}: {type_}")

    def add_field_to_impl(impl, param, need):
        # Exclusion for Data Set and Identifier
        # Used to set Command Dataset Type field
        if param in ["Data Set", "Identifier"]:
            need = row.get(column_name)
            if need.startswith('M'):
                impl.append(
                    "DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0001))"
                )
            else:
                impl.append(
                    "DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101))"
                )
            return
        # Convert parameter name to Rust field name (snake_case)
        field_name = param.lower().replace(' ', '_').replace('-', '_')
        if field_name == "priority":
            impl.append(
                f"DE::new(tags::{field_name.upper()}, VR::{vr}, value!(self.{field_name} as u16))"
            )
        else:
            impl.append(
                f"DE::new(tags::{field_name.upper()}, VR::{vr}, value!(self.{field_name}))"
            )

    for (prefix, (overview, *tag_def_ids)) in table_groups:
        merged = get_merged_df(table_list, overview, *tag_def_ids)
        for (column_name, suffix) in zip(
            [REQ_COLUMN, RESP_COLUMN, CANCEL_COLUMN],
            [REQ_SUFFIX, RESP_SUFFIX, CANCEL_SUFFIX]
        ):
            if column_name not in merged.columns:
                continue

            struct_name = f"{prefix.replace('_', '')}{suffix}"
            command_field_name = f"{prefix.upper()}_{suffix.upper()}"
            if suffix == 'Cncl':
                struct_name = f"{prefix.replace('_', '')}Cncl"
                command_field_name = "C_CANCEL_RQ"

            # Generate struct for request
            struct = []

            # Generate Command impl for request
            impl = []

            # Add fields from the merged dataframe
            for _, row in merged.iterrows():
                param = row['param']
                if "Sub-operations" in param:
                    param = param.replace("Sub-operations", "Suboperations")
                if column_name not in row:
                    raise RuntimeError(f"{column_name} not present for param {param}")
                if VR_COLUMN not in row:
                    raise RuntimeError(f"{VR_COLUMN} not present for param")

                need = row.get(column_name)
                vr = row.get(VR_COLUMN)
                # Add field with documentation from description field
                # Fall back to param name if no description
                description = row.get(DESCRIPTION_COLUMN, param).replace('\n\r', ' ')  
                add_field_to_impl(impl, param, need)
                add_field_to_struct(struct, param, need, vr, description)
            
            struct_text = f"#[derive(Builder)]\npub struct {struct_name}<'a> {{\n"
            struct_text += textwrap.indent(',\n'.join(struct), '    ')
            struct_text += "\n}"
            impl_text = f"""
impl<'a> Command for {struct_name}<'a> {{
    fn command_field(&self) -> u16 {{
        CommandField::{command_field_name} as u16
    }}

    #[rustfmt::skip]
    fn dataset(&self) -> InMemDicomObject {{
        InMemDicomObject::from_element_iter(vec![
"""
            impl_text += textwrap.indent(',\n'.join(impl), '            ')
            impl_text += "\n        ])\n"
            impl_text += "    }\n"
            impl_text += "}"
            generated += struct_text
            generated += impl_text
            generated += "\n"
            print(f"Generated struct {struct_name}")
    generated = f"""
use dicom_core::{{DataElement as DE, VR, dicom_value as value}};
use dicom_object::{{InMemDicomObject}};
use bon::Builder;
use dicom_dictionary_std::tags;
use crate::pdu::commands::{{CommandField, Priority}};
use crate::pdu::commands::Command;

""" + generated
    return generated
        
        
def write_rust_file(content, filename="generated.rs"):
    with open(Path.cwd() / f"src/pdu/{filename}", 'w') as f:
        f.write("// Auto-generated DICOM command structs\n")
        f.write(content)
    print(f"Generated Rust structs written to {filename}")

if __name__ == "__main__":
    rust_code = gen()
    write_rust_file(rust_code)


