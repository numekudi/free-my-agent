use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

const DEFAULT_PATTERNS: &str = "\
CLAUDE.md
AGENTS.md
.claude
.gemini
.agents
.github/copilot-instructions.md
.github/instructions/*
.github/prompts/*
.github/skills/*
";

pub fn install() -> Result<()> {
    let binary_path = resolve_binary()?;

    let git_dir = find_git_dir()?;
    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    install_hook(
        &hooks_dir.join("pre-commit"),
        &format!("{binary_path} restore"),
    )?;
    install_hook(
        &hooks_dir.join("post-commit"),
        &format!("{binary_path} free"),
    )?;

    install_default_patterns(&git_dir)?;

    println!("hooks installed in {}", hooks_dir.display());
    Ok(())
}

fn install_default_patterns(git_dir: &std::path::Path) -> Result<()> {
    let path = git_dir.join("free-my-agent");
    if path.exists() {
        return Ok(());
    }
    fs::write(&path, DEFAULT_PATTERNS)?;
    println!("default patterns written to {}", path.display());
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let git_dir = find_git_dir()?;
    let hooks_dir = git_dir.join("hooks");

    uninstall_hook(&hooks_dir.join("pre-commit"))?;
    uninstall_hook(&hooks_dir.join("post-commit"))?;

    println!("hooks removed");
    Ok(())
}

fn uninstall_hook(hook_path: &Path) -> Result<()> {
    if !hook_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(hook_path)?;
    let mut new_lines: Vec<&str> = Vec::new();
    let mut skip_next = false;
    for line in content.lines() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if line.starts_with("# free-my-agent:") {
            skip_next = true;
            continue;
        }
        new_lines.push(line);
    }

    let trimmed = new_lines.join("\n").trim_end().to_string();
    if trimmed.is_empty() || trimmed == "#!/bin/sh" {
        fs::remove_file(hook_path)?;
    } else {
        fs::write(hook_path, format!("{trimmed}\n"))?;
    }
    Ok(())
}

fn resolve_binary() -> Result<String> {
    let out = Command::new("which")
        .arg("free-my-agent")
        .output()
        .context("failed to run which")?;
    if out.status.success() {
        let path = String::from_utf8(out.stdout)?.trim().to_string();
        return Ok(path);
    }
    // fall back to current executable path
    let exe = std::env::current_exe().context("cannot determine current executable path")?;
    Ok(exe.to_string_lossy().to_string())
}

fn find_git_dir() -> Result<std::path::PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .context("failed to run git")?;
    anyhow::ensure!(out.status.success(), "not inside a git repository");
    let path = String::from_utf8(out.stdout)?.trim().to_string();
    Ok(std::path::PathBuf::from(path))
}

fn install_hook(hook_path: &Path, line: &str) -> Result<()> {
    let marker = format!("# free-my-agent: {line}");

    let existing = if hook_path.exists() {
        fs::read_to_string(hook_path)?
    } else {
        String::new()
    };

    if existing.contains(&marker) {
        println!("hook already present: {}", hook_path.display());
        return Ok(());
    }

    let content = if existing.is_empty() {
        format!("#!/bin/sh\n{marker}\n{line}\n")
    } else {
        format!("{existing}\n{marker}\n{line}\n")
    };

    fs::write(hook_path, &content)?;

    let mut perms = fs::metadata(hook_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(hook_path, perms)?;

    Ok(())
}
