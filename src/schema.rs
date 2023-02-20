use compact_str::CompactString;
use serde::{Deserialize};
use smallvec::SmallVec;


/// Root structure encompassing an entire tag list file.
#[derive(Debug, Deserialize)]
pub struct FileRoot {
    pub schema: Params,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Deserialize)]
pub struct Params {
    pub version: CompactString,
    pub namespace: CompactString,
}

#[derive(Debug, Deserialize)]
pub struct Tag {
    pub id: CompactString,
    pub description: String,
    pub attributes: Option<SmallVec<[Attribute; 4]>>,
    pub children: Option<SmallVec<[Child; 4]>>,
    pub value: Option<String>,
    pub example: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Attribute {
    pub id: CompactString,
    pub brief: CompactString,
    pub description: Option<String>,
    pub expected: Option<CompactString>,
    pub default: Option<CompactString>,
    pub optional: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Child {
    pub r#ref: CompactString,
    pub optional: Option<bool>,
    pub multiple: Option<bool>,
}
