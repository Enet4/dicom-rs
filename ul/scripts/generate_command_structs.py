import requests
from bs4 import BeautifulSoup
import pandas as pd
import textwrap
from pathlib import Path

# Label of dicom standard column that corresponds to parameter description
DESCRIPTION_COLUMN = "Description of Field"
# Label of dicom standard column that corresponds to the message request
REQ_COLUMN="Req/Ind"
# Label of dicom standard column that corresponds to the message response
RESP_COLUMN="Rsp/Conf"
# Label of dicom standard column that corresponds to the message cancel
CANCEL_COLUMN="CnclReq/CnclInd"
# Label of dicom standard column that corresponds to the parameter VR
VR_COLUMN="VR"
# Suffix for naming structs that represent Request messages
REQ_SUFFIX="Rq"
# Suffix for naming structs that represent Response messages
RESP_SUFFIX="Rsp"
# Suffix for naming structs that represent Cancel messages
CANCEL_SUFFIX="Cncl"

# Mapping from VR types to Rust types
VR_MAP = {
    "US": "u16",
    "UI": "&'a str",
    "AE": "&'a str"
}

# Groups for each type of message, contains 
# * The Message Group name
# * A list of tables that describe that message group
# 
# Each message group has an overview table that describes which
# fields are required and mandatory for each of the messages,
# which can either be Request, Response, or Cancel
#
# Then there are tables describing the parameters for each of
# of those messages in the message group
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

# Download dicom part7 (Message Exchange) and return all tables
def get_tables():
    url = "https://dicom.nema.org/medical/dicom/current/output/html/part07.html"
    response = requests.get(url)
    soup = BeautifulSoup(response.text, "html.parser")
    table_list = soup.find_all("div", {"class", "table"})
    return table_list

# Find a particular table by ID from the list of tablees
def find_table(table_list, table_id):
    tables = {}
    for table in table_list:
        table_def = table.contents[1]
        if table_def.has_attr("id") and table_def['id'] == table_id:
            return table

# Convert an html table to a pandas dataframe
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
    """Merge the tables describing each message group (see description of tables above)
    """
    overview = df_for_table(find_table(table_list, overview_id))
    # Get the definitions for tags for each specific command
    tag_defs = [df_for_table(find_table(table_list, table)) for table in tag_def_ids]
    # Add the values of all possible tags together
    tag_def = pd.concat(tag_defs).drop_duplicates()
    tag_def = tag_def.dropna()
    # Join now the overview table with the tags, dropping duplicate tag names
    full = overview.join(tag_def).reset_index().drop_duplicates(subset=['param'])
    return full

# Generate code
def gen():
    table_list = get_tables()
    generated = ""

    def add_field_to_struct(struct, param, need, vr, description):
        """Add a particular parameter to the struct definition 
        """
        # Convert parameter name to Rust field name (snake_case)
        field_name = param.lower().replace(' ', '_').replace('-', '_')
        # Exclusions: Don't add dataset/identifier to struct
        # and priority has a fixed field.
        if param in ["Data Set", "Identifier"]:
            return
        if need == '-':
            return
        if param == 'Priority':
            struct.append(
                "/// Priority for the request\n"
                "#[builder(default = Priority::Medium)]\n"
                "priority: Priority"
            )
            return
        # Try to get Rust type given `vr` in the table
        if vr not in VR_MAP:
            raise RuntimeError(f"No type found for VR {vr}: row {row}")
        value_type = VR_MAP.get(vr)
        if need[0] == 'M':
            type_ = f"{value_type}"
        else:
            type_ = f"Option<{value_type}>"
        # Add field to the struct
        struct.append(f"/// {description}")
        struct.append(f"pub {field_name}: {type_}")

    def add_field_to_dataset(impl, markers, param, need):
        """Add a particular parameter to the dataset representation of the struct
        """
        # Exclusion for Data Set and Identifier
        # Used to set Command Dataset Type field
        if param in ["Data Set", "Identifier"]:
            if need.startswith('M'):
                impl.append(
                    "DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0001))"
                )
                markers.append("DatasetRequiredCommand")
            elif need[0] in ['U', 'C']:
                markers.append("DatasetConditionalCommand")
                markers.append("DatasetRequiredCommand")
                impl.append(
                    "DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101))"
                )
            else:
                markers.append("DatasetForbiddenCommand")
                impl.append(
                    "DE::new(tags::COMMAND_DATA_SET_TYPE,VR::US, value!(0x0101))"
                )
            return
        if need == '-':
            return
        # Convert parameter name to Rust field name (snake_case)
        field_name = param.lower().replace(' ', '_').replace('-', '_')
        # Add the field to the implementation of the struct
        # special handling for `Priority` which needs a u16 cast-
        if field_name == "priority":
            impl.append(
                f"DE::new(tags::{field_name.upper()}, VR::{vr}, value!(self.{field_name} as u16))"
            )
        else:
            impl.append(
                f"DE::new(tags::{field_name.upper()}, VR::{vr}, value!(self.{field_name}))"
            )

    # Go through each table group
    for (prefix, (overview, *tag_def_ids)) in table_groups:
        # Get the merged table describing the group
        merged = get_merged_df(table_list, overview, *tag_def_ids)
        # Loop through each of the message types (Request, Response, Cancel)
        for (column_name, suffix) in zip(
            [REQ_COLUMN, RESP_COLUMN, CANCEL_COLUMN],
            [REQ_SUFFIX, RESP_SUFFIX, CANCEL_SUFFIX]
        ):
            # If this message group doesn't have a particular message type
            # Just skip it, i.e. `c-store` does not have a corresponding cancel 
            # message type
            if column_name not in merged.columns:
                continue

            struct_name = f"{prefix.replace('_', '')}{suffix}"
            command_field_name = f"{prefix.upper()}_{suffix.upper()}"
            # Special handling for cancel message names
            if suffix == 'Cncl':
                struct_name = f"{prefix.replace('_', '')}Cncl"
                command_field_name = "C_CANCEL_RQ"

            # Generate struct for request
            struct = []

            # Generate Command impl for request
            impl = []

            # Generated marker trait implementations
            markers = []

            # Add fields from the merged dataframe
            for _, row in merged.iterrows():
                param = row['param']
                # Special handling for sub-operation related fields
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
                add_field_to_dataset(impl, markers, param, need)
                add_field_to_struct(struct, param, need, vr, description)
            # If no marker traits were added, means that Data Set or Identifier was not mentioned
            # Which means we should implement `DatasetForbiddenCommand`
            if len(markers) == 0:
                markers.append("DatasetForbiddenCommand")

            
            # Create a struct for this using generated struct name and text
            # representations of all the fields
            struct_text = f"#[derive(Builder, Debug)]\npub struct {struct_name}<'a> {{\n"
            struct_text += textwrap.indent(',\n'.join(struct), '    ')
            struct_text += "\n}"

            # Implement marker trait(s) for struct
            marker_text = '\n'.join(f"impl<'a> {trait} for {struct_name}<'a> {{}}" for trait in markers)

            # Create an implementation of the trait `Command` for this struct
            # Command has two required implementations, 
            # * `command_field()`: can reference from the enum in `ul/src/pdu/commands.rs`,
            # * `dataset`: For this, we just need to serialize each property of the struct
            #   into a InMemDicomObject. For that we can just join the individual data elements
            #   as text created in `add_field_to_dataset`
            
            impl_text = f"""
impl<'a> Command for {struct_name}<'a> {{
    fn command_field(&self) -> u16 {{
        CommandField::{command_field_name} as u16
    }}

    fn dataset(&self) -> InMemDicomObject {{
        InMemDicomObject::from_element_iter(vec![
            DE::new(tags::COMMAND_FIELD, VR::US, value!(self.command_field())),
"""
            impl_text += textwrap.indent(',\n'.join(impl), '            ')
            impl_text += "\n        ])\n"
            impl_text += "    }\n"
            impl_text += "}"
            to_add = struct_text + "\n" + impl_text + "\n\n" + marker_text + "\n\n"
            if suffix == 'Cncl':
                to_add = to_add.replace("<'a>", "")
            generated += to_add
            print(f"Generated struct {struct_name}")
    generated = f"""
use dicom_core::{{DataElement as DE, VR, dicom_value as value}};
use dicom_object::{{InMemDicomObject}};
use bon::Builder;
use dicom_dictionary_std::tags;
use crate::pdu::commands::{{
    CommandField, Priority, Command, DatasetRequiredCommand,
    DatasetConditionalCommand, DatasetForbiddenCommand
}};

""" + generated
    return generated
        
        
def write_rust_file(content, filename="generated.rs"):
    with open(Path.cwd().parents[0] / f"src/pdu/{filename}", 'w') as f:
        f.write("// Auto-generated DICOM command structs\n")
        f.write(content)
    print(f"Generated Rust structs written to {filename}")

if __name__ == "__main__":
    rust_code = gen()
    write_rust_file(rust_code)


