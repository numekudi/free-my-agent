use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

use crate::config::{backup_dir, repo_root, resolve_targets};

pub fn free() -> Result<()> {
    let backup = backup_dir()?;
    let root = repo_root()?;
    let targets = resolve_targets()?;
    if targets.is_empty() {
        println!("no managed files found in this repository");
        return Ok(());
    }
    for target in &targets {
        let rel = target
            .strip_prefix(&root)
            .with_context(|| format!("{} is outside repo root", target.display()))?;
        let dest = backup.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_to(target, &dest)
            .with_context(|| format!("failed to backup {}", target.display()))?;
        remove_path(target).with_context(|| format!("failed to remove {}", target.display()))?;
        println!("freed: {}", rel.display());
    }
    Ok(())
}

pub fn restore() -> Result<()> {
    let backup = backup_dir()?;
    let root = repo_root()?;
    let patterns = crate::config::read_patterns()?;

    for entry in WalkDir::new(&backup).min_depth(1) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        let src = entry.path();
        let rel = src
            .strip_prefix(&backup)
            .context("backup path strip failed")?;
        let rel_str = rel.to_string_lossy();

        // A pattern like `.claude` should restore all files inside `.claude/`
        let matched = std::iter::once(rel).chain(rel.ancestors()).any(|ancestor| {
            let s = ancestor.to_string_lossy();
            patterns.iter().any(|p| {
                glob::Pattern::new(p)
                    .map(|pat| pat.matches(&s))
                    .unwrap_or(false)
            })
        });
        if !matched {
            continue;
        }

        let dest = root.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_to(src, &dest).with_context(|| format!("failed to restore {rel_str}"))?;
        fs::remove_file(src)?;

        Command::new("git")
            .args(["add", &dest.to_string_lossy()])
            .current_dir(&root)
            .status()
            .with_context(|| format!("failed to git add {rel_str}"))?;

        println!("restored: {rel_str}");
    }
    Ok(())
}

fn copy_to(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        if dst.exists() {
            fs::remove_dir_all(dst)?;
        }
        Command::new("cp")
            .args(["-r", &src.to_string_lossy(), &dst.to_string_lossy()])
            .status()
            .context("cp -r failed")?;
    } else {
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}
