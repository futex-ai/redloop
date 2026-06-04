//! Developer task runner entrypoint for the Redloop workspace.

#![warn(unreachable_pub)]

use std::env;
use std::process::ExitCode;

use crate::check::run_check;
use crate::review::run_review;
use crate::rust_file_length_lint::run_rust_file_length_lint;

mod check;
mod review;
mod rust_file_length_lint;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return Err("usage: cargo xtask <check|review|rust-file-length-lint>".to_owned());
    };
    let rest = args.collect::<Vec<_>>();
    match command.as_str() {
        "check" if rest.is_empty() => run_check(),
        "review" if rest.is_empty() => run_review(),
        "rust-file-length-lint" => run_rust_file_length_lint(&rest),
        _ => Err(format!("unknown or invalid xtask command `{command}`")),
    }
}
