#![allow(dead_code)]

//! # mdbook-xmldoc
//!
//! This binary crate provides a joint utility which serves both as a standalone
//! tool and a preprocessor for the `mdBook` static documentation generator.

use std::fs::File;

mod schema;

fn main() {
    log::info!("Hello world!");

    let mut reader = File::open("./temp/workspace.yml").unwrap();
    let root: schema::FileRoot = serde_yaml::from_reader(&mut reader).unwrap();

    dbg!(root);

}
