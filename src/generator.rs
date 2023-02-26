use std::cell::RefCell;
use std::io;
use std::fmt;

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
            },
            GeneratorError::InternalInputOutput { inner, description } => {
                f.write_fmt(format_args!("internal input/output error: {}", inner))?;
                match description {
                    Some(desc) => f.write_fmt(format_args!(", description: {}", desc)),
                    None => Ok(()),
                }
            },
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

}

impl fmt::Display for HeaderLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}


/// Generate Markdown content into `formatter` from the `root` tag list using the given `options`.
pub fn generate<'a>(root: &'a model::TagList, options: &'a GeneratorOptions,
                formatter: &'a mut fmt::Formatter<'a>) -> GeneratorResult<()>
{
    let context = Context {
        options,
        writer: RefCell::new(formatter),
        newline: if options.crlf { "\r\n" } else { "\n" },
        newblock: if options.crlf { "\r\n\r\n" } else { "\n\n" },
    };

    for (uuid, tag) in &root.tags {
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
        }

        if let Some(value) = &tag.value {
            context.write_tag_subheader("Value")?;
            context.write_paragraph(value)?;
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
                                context.write_tag_link(&root.namespace, name)?;
                            },
                            None => {
                                log::warn!("failed to resolve parent name for {} -> {}", uuid, parent_uuid);
                                continue 'parents;
                            }
                        };
                    }
                },
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
    writer: RefCell<&'a mut fmt::Formatter<'a>>,
    newline: &'static str,
    newblock: &'static str,
}

//noinspection RsBorrowChecker  - clion why
impl<'a> Context<'a> {
    pub fn writer_tag_header(&self, namespace: &str, title: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        writer.write_str("### `")?;
        writer.write_str(namespace)?;
        writer.write_str(":")?;
        writer.write_str(title)?;
        writer.write_str("`")?;
        writer.write_str(self.newblock)?;
        Ok(())
    }

    pub fn write_tag_subheader(&self, text: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        writer.write_str("_**")?;
        writer.write_str(text)?;
        writer.write_str(":**_")?;
        writer.write_str(self.newblock)?;
        Ok(())
    }

    pub fn write_paragraph(&self, text: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        writer.write_str(text)?;
        writer.write_str(self.newblock)?;
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

        writer.write_str("* `")?;
        writer.write_str(name)?;
        writer.write_str("` - ")?;
        writer.write_str(brief)?;
        if optional {
            writer.write_str(" _(optional)_")?;
        }
        writer.write_str(self.newline)?;

        if let Some(desc) = desc {
            writer.write_str("  * ")?;
            writer.write_str(desc)?;
            writer.write_str(self.newline)?;
        }

        if let Some(expected) = expected {
            writer.write_str("  * _Expected value:_ ")?;
            writer.write_str(expected)?;
            writer.write_str(self.newline)?;
        }

        if let Some(r#default) = r#default {
            writer.write_str("  * _Default value:_ ")?;
            writer.write_str(r#default)?;
            writer.write_str(self.newline)?;
        }

        writer.write_str(self.newline)?;
        Ok(())
    }

    pub fn write_tag_link(&self, namespace: &str, name: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        writer.write_str("* [`")?;
        writer.write_str(namespace)?;
        writer.write_str(":")?;
        writer.write_str(name)?;
        writer.write_str("`](#")?;
        writer.write_str(&namespace.to_lowercase())?;
        writer.write_str(&name.to_lowercase())?;
        writer.write_str(")")?;
        Ok(())
    }

    pub fn write_xml(&self, code: &str) -> GeneratorResult<()> {
        let mut writer = self.writer.borrow_mut();
        writer.write_str("```xml")?;
        writer.write_str(self.newline)?;
        writer.write_str(code)?;
        writer.write_str("```")?;
        writer.write_str(self.newblock)?;
        Ok(())
    }
}
