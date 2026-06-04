//! Read-only AI review command.

use std::ffi::OsStr;
use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};

pub(crate) fn run_review() -> Result<(), String> {
    let context = review_context()?;
    if context.trim().is_empty() {
        println!("No changes to review relative to origin/main or in the working tree.");
        return Ok(());
    }
    run_codex_review(&review_prompt(&context))
}

fn review_context() -> Result<String, String> {
    let sections = [
        (
            "branch diff stat",
            command_output("git", ["diff", "--stat", "origin/main...HEAD"])?,
        ),
        (
            "staged diff stat",
            command_output("git", ["diff", "--stat", "--cached"])?,
        ),
        (
            "unstaged diff stat",
            command_output("git", ["diff", "--stat"])?,
        ),
        (
            "untracked files",
            bounded_untracked_files(&command_output(
                "git",
                ["ls-files", "--others", "--exclude-standard"],
            )?),
        ),
    ];
    let mut context = String::new();
    for (label, value) in sections {
        if value.trim().is_empty() {
            continue;
        }
        context.push_str(label);
        context.push('\n');
        context.push_str(value.trim());
        context.push_str("\n\n");
    }
    Ok(context)
}

fn bounded_untracked_files(files: &str) -> String {
    const MAX_UNTRACKED_FILES: usize = 200;
    let paths = files.lines().collect::<Vec<_>>();
    let mut output = paths
        .iter()
        .take(MAX_UNTRACKED_FILES)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    let omitted = paths.len().saturating_sub(MAX_UNTRACKED_FILES);
    if omitted > 0 {
        output.push_str(&format!(
            "\n... {omitted} additional untracked files omitted"
        ));
    }
    output
}

fn review_prompt(context: &str) -> String {
    format!(
        "You are Codex, an AI code reviewer performing a read-only review of this local change set.\n\nReview scope:\n- Base ref: origin/main.\n- Include committed branch changes, staged changes, unstaged changes, and untracked files.\n- The repository is available at the current working directory.\n\nLocal change summary:\n{context}\nInstructions:\n- Inspect the changed files and nearby dependencies before reporting findings.\n- Focus on bugs, behavioral regressions, missing tests, stale docs, security, and operational risks.\n- Do not make edits.\n- If there are no material issues, say so clearly and mention any residual test or review risk.\n- Report findings first, ordered by severity, with file and line references when available.\n"
    )
}

fn run_codex_review(prompt: &str) -> Result<(), String> {
    let mut child = Command::new("codex")
        .args([
            "--ask-for-approval",
            "never",
            "exec",
            "--ephemeral",
            "--ignore-rules",
            "--model",
            "gpt-5.5",
            "--config",
            "model_reasoning_effort=\"xhigh\"",
            "--sandbox",
            "read-only",
            "--skip-git-repo-check",
            "--cd",
            ".",
            "-",
        ])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|source| format!("failed to start `cargo xtask review`: {source}"))?;
    let write_result = {
        let Some(mut stdin) = child.stdin.take() else {
            return Err("failed to open review subprocess stdin".to_owned());
        };
        stdin.write_all(prompt.as_bytes())
    };
    if let Err(source) = write_result
        && source.kind() != ErrorKind::BrokenPipe
    {
        return Err(format!("failed to write review prompt: {source}"));
    }
    let status = child
        .wait()
        .map_err(|source| format!("failed to wait for review subprocess: {source}"))?;
    if status.success() {
        return Ok(());
    }
    Err(format!("review subprocess exited with {status}"))
}

fn command_output<I, S>(program: &str, args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|source| format!("failed to run `{program}`: {source}"))?;
    if !output.status.success() {
        return Err(format!("`{program}` exited with {}", output.status));
    }
    String::from_utf8(output.stdout).map_err(|source| format!("stdout was not utf-8: {source}"))
}
