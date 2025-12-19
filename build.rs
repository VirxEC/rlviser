use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

const SCHEMA_DIR: &str = "./spec";
const OUT_FILE: &str = "./src/flat.rs";

/// Taken from <https://github.com/planus-org/planus/blob/main/crates/planus-codegen/src/rust/mod.rs#L1014>
///
/// This formats a string using `rustfmt` (using Rust 2024 and not 2021)
fn format_string(s: &str) -> String {
    let mut child = Command::new("rustfmt");

    child
        .arg("--edition=2024")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = child.spawn().expect("Unable to spawn rustfmt. Perhaps it is not installed?");

    {
        let child_stdin = child.stdin.as_mut().unwrap();
        child_stdin
            .write_all(s.as_bytes())
            .expect("Unable to write the file to rustfmt");
    }

    let output = child
        .wait_with_output()
        .expect("Unable to get the formatted file back from rustfmt");

    if output.status.success() && output.stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).into_owned()
    } else if output.stderr.is_empty() {
        panic!("rustfmt failed with exit code {}", output.status);
    } else {
        panic!(
            "rustfmt failed with exit code {} and message:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

fn main() {
    println!("cargo:rerun-if-changed=./spec");
    println!("cargo:rerun-if-changed=build.rs");

    let fbs_path = PathBuf::from(SCHEMA_DIR).join("core.fbs");
    let declarations = planus_translation::translate_files(&[fbs_path.as_path()]).unwrap();
    let raw_out = planus_codegen::generate_rust(&declarations)
        .unwrap()
        .replace("#[no_implicit_prelude]\n", "")
        .replace("::serde::Serialize,", "")
        .replace("::serde::Deserialize,", "")
        .replace("::serde::Deserialize", "")
        .replace(
            "#[derive(Clone, Debug, PartialEq, PartialOrd,  )]\n        pub struct GameState {",
            "#[derive(::bevy::prelude::Resource, Clone, Debug, PartialEq, PartialOrd)]\npub struct GameState {",
        )
        .replace(
            "#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,  )]\n        #[repr(u8)]\n        pub enum GameMode {",
            "#[derive(::bevy::prelude::Resource, Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]\n#[repr(u8)]\npub enum GameMode {"
        )
        .replace(
            "TheVoid = 5,",
            "#[default]\nTheVoid = 5,"
        );

    fs::write(OUT_FILE, format_string(&raw_out).as_bytes()).unwrap();
}
