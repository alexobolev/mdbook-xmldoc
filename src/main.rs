#![allow(dead_code)]

//! # mdbook-xmldoc
//!
//! This binary crate provides a joint utility which serves both as a standalone
//! tool and an `mdBook` preprocessor for generating simplistic static XML document
//! reference in an opinionated markdown format.

mod generator;
mod model;
mod schema;

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

use clap::{Parser, Subcommand};

use crate::model::loader;


#[derive(Debug, Parser)]
#[command(author, version, about)]
#[command(propagate_version = true)]
struct Cli {
    /// Provide additional diagnostics. DISABLE FOR MDBOOK!
    #[arg(long)]
    verbose: bool,
    /// Disable colored logging, useful when piping output to files.
    #[arg(long)]
    no_colors: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Checks that a given file is a valid .yml tag list.
    Check { file: PathBuf },
    /// Generates a pure markdown file from the given file.
    GenerateInto { file: PathBuf, output: PathBuf },
}


fn main() {
    let cli_args = Cli::parse();
    let log_dispatch = {
        let get_filter = |verbose| match verbose {
            false => log::LevelFilter::Info,
            true => log::LevelFilter::Trace,
        };

        let get_prefix = |level| match level {
            log::Level::Error => "Error: ",
            log::Level::Warn => "Warning: ",
            log::Level::Info => "",
            log::Level::Debug => "Debug info: ",
            log::Level::Trace => "TRACING: "
        };

        let no_colors = cli_args.no_colors;
        let colors = fern::colors::ColoredLevelConfig::new()
            .error(fern::colors::Color::Red)
            .warn(fern::colors::Color::Yellow)
            .trace(fern::colors::Color::BrightBlack);

        fern::Dispatch::new()
            .format(move |out, message, record| {
                let prefix = get_prefix(record.level());
                if !no_colors {
                    let color = colors.get_color(&record.level());
                    out.finish(format_args!("\x1B[{}m{}{} \x1B[0m",
                        color.to_fg_str(), prefix, message))
                } else {
                    out.finish(format_args!("{}{}", prefix, message))
                }
            })
            .level(get_filter(cli_args.verbose))
            .chain(io::stdout())
    };

    // TODO: Route warn and error logs to stderr.

    if let Err(err) = log_dispatch.apply() {
        eprintln!("failed to configure log: {}", err);
        eprintln!("exiting...");
        process::exit(3);
    }

    let success = match &cli_args.command {
        Command::Check { file } =>
            exec_check(file.as_path()),
        Command::GenerateInto { file, output } =>
            exec_generate(file.as_path(), output.as_path()),
    };

    if !success {
        log::error!("mdbook-xmldoc failed, check the logs!");
        if !cli_args.verbose {
            log::error!("if the logs are empty, run with --verbose");
        }
        process::exit(1);
    }
}


fn exec_check(path: &Path) -> bool {
    if let Some(loader::LoadDigest { warnings, .. }) = internal_load(path) {
        for warning in &warnings {
            log::warn!("warning: {}", warning);
        }

        let warning_count = warnings.len();
        match warning_count {
            0 => log::info!("file ok"),
            _ => log::info!("file has warning(s): {}", warning_count),
        };

        true
    } else {
        false
    }
}

fn exec_generate(path: &Path, output: &Path) -> bool {
    if let Some(loader::LoadDigest { model, warnings }) = internal_load(path) {
        for warning in &warnings {
            log::warn!("warning: {}", warning);
        }

        let options = generator::GeneratorOptions {
            level: generator::HeaderLevel::new(1).unwrap(),
            crlf: false,
        };

        let generator_result = if output.to_string_lossy() == "(stdout)" {
            generator::generate(&model, &options, &mut io::stdout())
        } else {
            match File::create(output) {
                Ok(file) => {
                    let mut writer = io::BufWriter::new(file);
                    generator::generate(&model, &options, &mut writer)
                },
                Err(error) => {
                    log::error!("failed to create or truncate output file: {}", error);
                    return false;
                }
            }
        };

        match generator_result {
            Ok(()) => true,
            Err(error) => {
                log::error!("failed to generate markdown: {}", error);
                false
            }
        }
    } else {
        false
    }
}


fn internal_load(path: &Path) -> Option<loader::LoadDigest> {
    let mut reader = match File::open(path) {
        Ok(file) => file,
        Err(err) => {
            log::error!("failed to open source file '{}'", path.to_string_lossy());
            log::error!("error: {}", err.to_string());
            return None;
        }
    };

    let root: schema::FileRoot = match serde_yaml::from_reader(&mut reader) {
        Ok(root) => root,
        Err(err) => {
            log::error!("failed to parse tag list from source file '{}'", path.to_string_lossy());
            log::error!("error: {}", err.to_string());
            return None;
        }
    };

    match loader::load_from(root) {
        Ok(digest) => Some(digest),
        Err(error) => {
            log::error!("failed to load model from deserialized schema '{}'", path.to_string_lossy());
            log::error!("reason: {:?}", error);
            None
        }
    }
}
