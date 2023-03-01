use std::collections::HashMap;
use compact_str::CompactString;
use smallvec::SmallVec;
use uuid::Uuid;


/// The latest schema identifier implemented by this version of `mdbook-xmldoc`.
pub const VERSION: &str = "r1";

/// Check if a schema version is supported by this identifier.
pub fn is_supported(version: &str) -> bool {
    let lower = version.to_ascii_lowercase();
    matches!(lower.trim(), VERSION)
}


/// Root structure of a mutable pre-processed tag list.
#[derive(Debug, Default)]
pub struct TagList {
    /// The XML namespace of all tags in this list.
    pub namespace: CompactString,
    /// Tag descriptions within this list.
    pub tags: HashMap<Uuid, Tag>,
    /// Mapping between tag names and internal ids.
    pub names: HashMap<CompactString, Uuid>,
    /// Lookup for child -> parent tag relations.
    pub parents: HashMap<Uuid, SmallVec<[Uuid; 4]>>,
}

/// Description of a tag.
#[derive(Debug, Default)]
pub struct Tag {
    /// Internal identifier of this tag.
    pub id: Uuid,
    /// Public tag name.
    pub name: CompactString,
    /// Mandatory description.
    pub description: String,
    /// The attributes this tag may have.
    pub attributes: SmallVec<[Attribute; 4]>,
    /// The child tags this tag may contain.
    pub children: SmallVec<[Child; 4]>,
    /// The scalar value this tag may contain.
    pub value: Option<String>,
    /// An abstract XML example code demonstrating this tag.
    pub example: Option<String>,
    /// Order of the tag definition in its source file.
    index_internal: i32,
}
impl Tag {
    #[inline]
    pub fn index(&self) -> i32 {
        self.index_internal
    }
}

/// Description of an allowed (or expected) tag attribute.
#[derive(Debug, Default)]
pub struct Attribute {
    /// Attribute name.
    pub name: CompactString,
    /// Mandatory brief description.
    pub short_description: CompactString,
    /// Optional long description (may have paragraphs).
    pub long_description: Option<String>,
    /// Flag showing whether the attribute can be omitted.
    pub is_optional: bool,
    /// What kind of value the schema expects this attribute to have?
    pub expected_value: Option<CompactString>,
    /// The default value this tag would have if it `is_optional`.
    pub default_value: Option<CompactString>,
}

/// Description of a tag (subject) which may be used within another tag (parent).
#[derive(Debug, Default)]
pub struct Child {
    /// (Hopefully) resolved tag name reference.
    pub reference: ChildInternal,
    /// Can the parent tag have no instances of the subject tag?
    pub is_optional: bool,
    /// Can the parent tag have multiple instances of the subject tag?
    pub is_repeatable: bool,
}

#[derive(Debug)]
pub enum ChildInternal {
    Resolved { id: Uuid },
    Unresolved { name: CompactString },
}
impl Default for ChildInternal {
    fn default() -> Self {
        Self::Unresolved { name: "".into() }
    }
}



/// Encapsulation of [`super::model`] loading logic.
pub mod loader {
    use smallvec::smallvec;
    use super::*;


    /// Loaded [`TagList`] model with possible warnings.
    pub struct LoadDigest {
        /// Valid tag list model.
        pub model: TagList,
        /// Non-fatal issues.
        pub warnings: SmallVec<[String; 4]>,
    }
    impl LoadDigest {
        pub fn has_warnings(&self) -> bool {
            !self.warnings.is_empty()
        }
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
            tags: HashMap::new(),
            names: HashMap::new(),
            parents: HashMap::new(),
        };

        let tag_count = schema.tags.len();
        tl_root.names.reserve(tag_count);
        tl_root.tags.reserve(tag_count);
        tl_root.parents.reserve(tag_count);

        if tl_root.namespace.is_empty() || !tl_root.namespace.is_ascii() {
            log::debug!("schema namespace must be a non-empty ascii sequence");
            // TODO: Consider using CompactString for tl_warnings, or a Cow<String>.
            tl_warnings.push(String::from("schema namespace must be a non-empty ascii sequence"));
        }

        // Tags are processed in multiple steps to avoid name resolution conflicts.
        //
        // First, everything that we can map from schema to model without issue is processed.
        //   During that process, all child relationships are stored in a temporary vector.
        // Second, we pre-build a name lookup - it will not be affected by child vectors.
        // Third, we process the temporary vector by mapping child tags into their parents.

        let mut children_temp = HashMap::new();

        debug_assert!(tl_root.names.is_empty());
        for (index, tag_schema) in schema.tags.into_iter().enumerate() {
            let mut tag = Tag {
                id: Uuid::new_v4(),
                name: tag_schema.id,
                description: tag_schema.description.trim().into(),
                attributes: Default::default(),  // <- still need to process attributes
                children: Default::default(),  // <- still need to process child tags
                value: tag_schema.value.map(|v| v.trim().into()),
                example: tag_schema.example,
                index_internal: index as i32 + 1,
            };

            tag.attributes = tag_schema.attributes
                .unwrap_or_else(|| smallvec![])
                .into_iter()
                .map(|attr_schema| {
                    Attribute {
                        name: attr_schema.id,
                        short_description: attr_schema.brief.trim().into(),
                        long_description: attr_schema.description.map(|d| d.trim().into()),
                        is_optional: attr_schema.optional.unwrap_or(false),
                        expected_value: attr_schema.expected.map(|ev| ev.trim().into()),
                        default_value: attr_schema.default.map(|dv| dv.trim().into()),
                    }
                })
                .collect();

            children_temp.insert(tag.id, tag_schema.children.unwrap_or_else(|| smallvec![]));

            if tl_root.tags.insert(tag.id, tag).is_some() {
                panic!("non-unique generated internal tag uuid?!");
            }
        }

        debug_assert!(tl_root.names.is_empty());
        for (uuid, tag) in &tl_root.tags {
            if tl_root.names.insert(tag.name.clone(), *uuid).is_some() {
                panic!("non-unique name -> uuid mapping?!");
            }
        }

        // At this point, we can use the uuid <-> name lookup
        // tables, which is needed for child processing.

        for (parent_uuid, child_schemas) in &children_temp {
            let parent_model = tl_root.tags.get_mut(parent_uuid)
                .expect("failed to resolve an internal parent reference");
            debug_assert!(parent_model.children.is_empty());

            for child_schema in child_schemas {
                let reference = match tl_root.names.get(&child_schema.r#ref) {
                    Some(child_uuid) => ChildInternal::Resolved { id: *child_uuid },
                    None => ChildInternal::Unresolved { name: child_schema.r#ref.clone() },
                };
                let child = Child {
                    reference,
                    is_optional: child_schema.optional.unwrap_or(false),
                    is_repeatable: child_schema.multiple.unwrap_or(false),
                };

                if let ChildInternal::Unresolved { name } = &child.reference {
                    tl_warnings.push(format!("unresolved child reference: {}->{}", parent_model.name, name));
                }

                if let ChildInternal::Resolved { id } = &child.reference {
                    if !tl_root.parents.contains_key(id) {
                        tl_root.parents.insert(*id, smallvec![]);
                    }
                    tl_root.parents.get_mut(id).unwrap().push(*parent_uuid);
                }

                parent_model.children.push(child);
            }
        }

        let root_pairs = tl_root.tags.values()
            .map(|tag| (tag.id, tag.name.clone()))
            .filter(|(id, _)| !tl_root.parents.contains_key(id))
            .collect::<SmallVec<[(Uuid, CompactString); 4]>>();

        match root_pairs.len() {
            1 => (),
            0 => {
                tl_warnings.push(String::from("schema has no root tags, likely self-referential?"))
            },
            c => {
                let names_list = root_pairs.into_iter()
                    .map(|(_, name)| name)
                    .collect::<SmallVec<[CompactString; 4]>>()
                    .join(", ");
                tl_warnings.push(format!("schema has more than one root tag ({}): {}", c, names_list))
            },
        };

        Ok(LoadDigest { model: tl_root, warnings: tl_warnings })
    }
}
