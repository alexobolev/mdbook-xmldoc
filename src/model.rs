use std::collections::HashMap;
use compact_str::CompactString;
use smallvec::SmallVec;
use uuid::Uuid;


/// The latest schema identifier implemented by this version of `mdbook-xmldoc`.
pub const VERSION: &str = "r1";

/// Check if a schema version is supported by this identifier.
pub fn is_supported(version: &str) -> bool {
    let lower = version.to_ascii_lowercase();
    match lower.trim() {
        VERSION => true,
        _ => false,
    }
}


/// Root structure of a mutable pre-processed tag list.
#[derive(Debug)]
pub struct TagList {
    /// The XML namespace of all tags in this list.
    namespace: CompactString,
    /// Mapping between tag names and internal ids.
    names: HashMap<CompactString, Uuid>,
    /// Tag descriptions within this list.
    tags: HashMap<Uuid, Tag>,
}

/// Description of a tag.
#[derive(Debug)]
pub struct Tag {
    /// Public tag name.
    name: CompactString,
    /// Mandatory description.
    description: String,
    /// The attributes this tag may have.
    attributes: SmallVec<[Attribute; 4]>,
    /// The child tags this tag may contain.
    children: SmallVec<[Child; 4]>,
    /// The scalar value this tag may contain.
    value: Option<String>,
    /// An abstract XML example code demonstrating this tag.
    example: Option<String>,
}

/// Description of an allowed (or expected) tag attribute.
#[derive(Debug)]
pub struct Attribute {
    /// Attribute name.
    name: CompactString,
    /// Mandatory brief description.
    short_description: CompactString,
    /// Optional long description (may have paragraphs).
    long_description: Option<String>,
    /// Flag showing whether the attribute can be omitted.
    is_optional: bool,
    /// What kind of value the schema expects this attribute to have?
    expected_value: Option<String>,
    /// The default value this tag would have if it `is_optional`.
    default_value: Option<String>,
}

/// Description of a tag (subject) which may be used within another tag (parent).
#[derive(Debug)]
pub struct Child {
    /// Unique identifier of the subject tag.
    id: Uuid,
    /// Can the parent tag have no instances of the subject tag?
    is_optional: bool,
    /// Can the parent tag have multiple instances of the subject tag?
    is_repeatable: bool,
}



/// Encapsulation of [`super::model`] loading logic.
pub mod loader {
    use super::*;


    /// Loaded [`TagList`] model with possible warnings.
    pub struct LoadDigest {
        /// Valid tag list model.
        pub model: TagList,
        /// Non-fatal issues.
        pub warnings: SmallVec<[String; 4]>,
    }

    /// Possible fatal errors produced by [`load_from`].
    #[derive(Debug)]
    pub enum LoadError {
        /// Schema version wasn't supported.
        VersionUnsupported { found: CompactString, expected: CompactString }
    }


    /// Load a [`TagList`] model from a deserialized `schema` instance.
    pub fn load_from(schema: crate::schema::FileRoot) -> Result<LoadDigest, LoadError> {
        let schema_version = schema.schema.version;
        if !is_supported(schema_version.as_str()) {
            return Err(LoadError::VersionUnsupported {
                found: schema_version,
                expected: CompactString::from(VERSION)
            });
        }

        let mut tl_warnings = SmallVec::new();
        let mut tl_root = TagList {
            namespace: schema.schema.namespace,
            names: HashMap::new(),
            tags: HashMap::new(),
        };

        let tag_count = schema.tags.len();
        tl_root.names.reserve(tag_count);
        tl_root.tags.reserve(tag_count);

        // Check the namespace (non-fatal).
        if !tl_root.namespace.is_empty() || !tl_root.namespace.is_ascii() {
            tl_warnings.push(format!("schema namespace must be a non-empty ascii sequence"));
        }

        todo!();
        Ok(LoadDigest { model: tl_root, warnings: tl_warnings })
    }
}
