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
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Checks that a given file is a valid .yml tag list.
    Check { file: PathBuf },
    /// Generates a pure markdown file from the given file.
    Generate { file: PathBuf, output: Option<PathBuf> },
}


fn main() {
    let cli_args = Cli::parse();
    let log_dispatch = fern::Dispatch::new()
        .format(|out, message, record| {
           out.finish(format_args!(
               "[{}][{}] {}",
               chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
               record.level(),
               message,
           ))
        })
        .level(if cli_args.verbose { log::LevelFilter::Debug } else { log::LevelFilter::Info })
        .chain(std::io::stdout());

    // TODO: Route warn and error logs to stderr.
    // TODO: Set up colored output when not piping out.

    if let Err(err) = log_dispatch.apply() {
        eprintln!("failed to configure log: {}", err);
        eprintln!("exiting...");
        process::exit(3);
    }

    let success = match &cli_args.command {
        Command::Check { file } =>
            exec_check(file.as_path()),
        Command::Generate { file, output } =>
            exec_generate(file.as_path(), output.as_deref().unwrap_or(file.as_path())),
    };

    if !success {
        log::error!("mdbook-xmldoc failed, check the logs!");
        log::error!("if the logs are empty, run with --verbose");
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

#[allow(unused_variables)]  // TODO: Remove this, temporary.
fn exec_generate(path: &Path, output: &Path) -> bool {
    if let Some(loader::LoadDigest { model, warnings }) = internal_load(path) {
        for warning in &warnings {
            log::warn!("warning: {}", warning);
        }

        // TODO: Actually use HeaderLevel in the generator.
        let options = generator::GeneratorOptions {
            level: generator::HeaderLevel::new(3).unwrap(),
            crlf: false,
        };

        match generator::generate(&model, &options, &mut std::io::stdout()) {
            Ok(_) => true,
            Err(e) => {
                log::error!("failed to generate markdown: {}", e);
                false
            },
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
            log::error!("error: {:?}", error);
            None
        }
    }
}
