use std::cell::RefCell;
use std::io;
use std::fmt;
use smallvec::SmallVec;

use super::model;


/// Specialization of [`Result`] over [`GeneratorError`].
/// Should be returned by all generator code.
pub type GeneratorResult<T> = Result<T, GeneratorError>;

/// Possible errors produced by generator code.
#[derive(Debug)]
pub enum GeneratorError {
    /// Required header level was out of allowed bounds (`[1..=6]`).
    BadHeaderLevel { level: i32 },
    /// Generator suffered a formatting error.
    InternalFormatting { inner: fmt::Error, description: Option<String> },
    /// Generator suffered an input/output error.
    InternalInputOutput { inner: io::Error, description: Option<String> },
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeneratorError::BadHeaderLevel { level } =>
                f.write_fmt(format_args!("invalid header level '{}'", level)),
            GeneratorError::InternalFormatting { inner, description } => {
                f.write_fmt(format_args!("internal formatting error: {}", inner))?;
                match description {
                    Some(desc) => f.write_fmt(format_args!(", description: {}", desc)),
                    None => Ok(()),
                }
            }
            GeneratorError::InternalInputOutput { inner, description } => {
                f.write_fmt(format_args!("internal input/output error: {}", inner))?;
                match description {
                    Some(desc) => f.write_fmt(format_args!(", description: {}", desc)),
                    None => Ok(()),
                }
            }
        }
    }
}

impl From<fmt::Error> for GeneratorError {
    fn from(inner: fmt::Error) -> Self {
        Self::InternalFormatting { inner, description: None }
    }
}

impl From<io::Error> for GeneratorError {
    fn from(inner: io::Error) -> Self {
        Self::InternalInputOutput { inner, description: None }
    }
}


/// Configuration struct passed to Markdown generator functions.
#[derive(Debug)]
pub struct GeneratorOptions {
    /// The starting heading level that the generator should descend from.
    pub level: HeaderLevel,
    /// Whether to use CRLF for new lines instead of LF.
    pub crlf: bool,
}


/// Checked Markdown / HTML heading level.
#[derive(Clone, Copy, Debug)]
pub struct HeaderLevel(i32);

impl HeaderLevel {
    /// Create a new checked [`HeaderLevel`] instance.
    pub fn new(level: i32) -> GeneratorResult<HeaderLevel> {
        match level {
            1..=6 => Ok(HeaderLevel(level)),
            _ => Err(GeneratorError::BadHeaderLevel { level }),
        }
    }

    /// Get a new heading level that is one unit deeper than [`self`].
    #[inline]
    pub fn next(&self) -> GeneratorResult<HeaderLevel> {
        Self::new(self.0 + 1)
    }

    /// Get the Markdown `#` prefix for this heading level.
    #[inline]
    pub fn get_prefix(&self) -> &'static str {
        match self.0 {
            1 => "#",
            2 => "##",
            3 => "###",
            4 => "####",
            5 => "#####",
            6 => "######",
            _ => panic!("invalid internal header level"),
        }
    }
}

impl fmt::Display for HeaderLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}


/// Generate Markdown content into `formatter` from the `root` tag list using the given `options`.
pub fn generate<'a>(root: &'a model::TagList, options: &'a GeneratorOptions,
                    formatter: &'a mut dyn io::Write) -> GeneratorResult<()>
{
    let context = Context {
        options,
        writer: RefCell::new(formatter),
        newline: if options.crlf { "\r\n" } else { "\n" },
        newblock: if options.crlf { "\r\n\r\n" } else { "\n\n" },
    };

    // Instead of preserving order on model construction, it is recovered here.
    // To consider: move this to model, or rework the model to preserve the order inherently.
    let ordered_tags = {
        type TagPair<'a> = (&'a uuid::Uuid, &'a model::Tag);
        let mut pairs: Vec<TagPair> = root.tags.iter().collect();
        pairs.sort_by(|a: &TagPair, b: &TagPair| {
            a.1.index().partial_cmp(&b.1.index()).unwrap()
        });
        pairs
    };

    for (uuid, tag) in ordered_tags {
        context.writer_tag_header(&root.namespace, &tag.name)?;
        context.write_paragraph(&tag.description)?;

        if !tag.attributes.is_empty() {
            context.write_tag_subheader("Attributes")?;
            for attr in &tag.attributes {
                context.write_attribute(
                    &attr.name,
                    &attr.short_description,
                    attr.long_description.as_deref(),
                    attr.is_optional,
                    attr.expected_value.as_deref(),
                    attr.default_value.as_deref(),
                )?;
            }
            context.write_newblock()?;
        }

        if let Some(value) = &tag.value {
            context.write_tag_subheader("Value")?;
            context.write_paragraph(value)?;
        }

        if !tag.children.is_empty() {
            context.write_tag_subheader("Children")?;
            for child in &tag.children {
                match &child.reference {
                    model::ChildInternal::Resolved { id } => {
                        context.write_child_item(
                            true,
                            &root.namespace,
                            &root.tags.get(id).unwrap().name,
                            child.is_optional,
                            child.is_repeatable,
                        )?;
                    },
                    model::ChildInternal::Unresolved { name } => {
                        context.write_child_item(
                            false,
                            &root.namespace,
                            name,
                            child.is_optional,
                            child.is_repeatable,
                        )?;
                    },
                };
            }
            context.write_newblock()?;
        }

        // Parent block is always present.
        {
            context.write_tag_subheader("Parents")?;
            match root.parents.get(uuid) {
                Some(parents) => {
                    'parents: for parent_uuid in parents {
                        match root.tags.get(parent_uuid) {
                            Some(parent_tag) => {
                                let name = parent_tag.name.as_str();
                                context.write_parent_item(&root.namespace, name)?;
                            }
                            None => {
                                log::warn!("failed to resolve parent name for {} -> {}", uuid, parent_uuid);
                                continue 'parents;
                            }
                        };
                    }
                    context.write_newblock()?;
                }
                None => context.write_paragraph("This tag has no possible parents!")?,
            }
        }

        if let Some(example) = &tag.example {
            context.write_tag_subheader("Example")?;
            context.write_xml(example)?;
        }
    }

    Ok(())
}

struct Context<'a> {
    options: &'a GeneratorOptions,
    writer: RefCell<&'a mut dyn io::Write>,
    newline: &'static str,
    newblock: &'static str,
}

//noinspection RsBorrowChecker  - clion why
impl<'a> Context<'a> {
    pub fn writer_tag_header(&self, namespace: &str, title: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        write!(writer, "{} `{}:{}`{}", self.options.level.get_prefix(), namespace, title, self.newblock)?;
        Ok(())
    }

    pub fn write_tag_subheader(&self, text: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        write!(writer, "_**{}:**_{}", text, self.newblock)?;
        Ok(())
    }

    pub fn write_paragraph(&self, text: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        write!(writer, "{}{}", text, self.newblock)?;
        Ok(())
    }

    pub fn write_attribute(&self,
                           name: &str,
                           brief: &str,
                           desc: Option<&str>,
                           optional: bool,
                           expected: Option<&str>,
                           r#default: Option<&str>) -> GeneratorResult<()>
    {
        let mut writer = self.writer.borrow_mut();

        let optional_text = if optional { " _(optional)_" } else { "" };
        write!(writer, "* `{}` - {}{}{}", name, brief, optional_text, self.newline)?;

        if let Some(desc) = desc {
            write!(writer, "  * {}{}", desc, self.newline)?;
        }

        if let Some(expected) = expected {
            write!(writer, "  * _Expected value:_ {}{}", expected, self.newline)?;
        }

        if let Some(r#default) = r#default {
            write!(writer, "  * _Default value:_ {}{}", r#default, self.newline)?;
        }

        Ok(())
    }

    pub fn write_parent_item(&self, namespace: &str, name: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        write!(writer, "* [`{}:{}`](#{}{}){}", namespace, name, &namespace.to_lowercase(), &name.to_lowercase(), self.newline)?;
        Ok(())
    }

    pub fn write_child_item(&self, linked: bool, namespace: &str, name: &str, optional: bool, repeated: bool) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        if linked {
            write!(writer, "* [`{}:{}`](#{}{})", namespace, name, &namespace.to_lowercase(), &name.to_lowercase())?;
        } else {
            write!(writer, "* `{}:{}`", namespace, name)?;
        }

        if optional || repeated {
            let mut modifiers = SmallVec::<[&'static str; 2]>::new();
            if optional { modifiers.push("optional"); }
            if repeated { modifiers.push("repeated"); }
            write!(writer, " _({})_", modifiers.join(", "))?;
        }

        write!(writer, "{}", self.newline)?;
        Ok(())
    }

    pub fn write_xml(&self, code: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        write!(writer, "```xml{}{}{}```{}", self.newline, code.trim_end(), self.newline, self.newblock)?;
        Ok(())
    }

    pub fn write_newblock(&self) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        write!(writer, "{}", self.newblock)?;
        Ok(())
    }
}
