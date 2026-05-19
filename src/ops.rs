use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

use crate::config::{backup_dir, repo_root, resolve_targets};

pub fn free() -> Result<()> {
    let backup = backup_dir()?;
    let targets = resolve_targets()?;
    if targets.is_empty() {
        println!("no managed files found in this repository");
        return Ok(());
    }
    for target in &targets {
        let name = target
            .file_name()
            .context("invalid path")?
            .to_string_lossy();
        let dest = backup.join(name.as_ref());
        copy_to(&target, &dest)
            .with_context(|| format!("failed to backup {}", target.display()))?;
        remove_path(target).with_context(|| format!("failed to remove {}", target.display()))?;
        println!("freed: {}", target.display());
    }
    Ok(())
}

pub fn restore() -> Result<()> {
    let backup = backup_dir()?;
    let root = repo_root()?;
    let patterns = crate::config::read_patterns()?;

    for entry in fs::read_dir(&backup)? {
        let entry = entry?;
        let src = entry.path();
        let name = src
            .file_name()
            .context("invalid path")?
            .to_string_lossy()
            .to_string();

        let matched = patterns.iter().any(|p| {
            glob::Pattern::new(p)
                .map(|pat| pat.matches(&name))
                .unwrap_or(false)
        });
        if !matched {
            continue;
        }

        let dest = root.join(&name);
        copy_to(&src, &dest).with_context(|| format!("failed to restore {name}"))?;

        Command::new("git")
            .args(["add", &dest.to_string_lossy()])
            .current_dir(&root)
            .status()
            .with_context(|| format!("failed to git add {name}"))?;

        println!("restored: {name}");
    }
    Ok(())
}

fn copy_to(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if src.is_dir() {
        Command::new("cp")
            .args(["-r", &src.to_string_lossy(), &dst.to_string_lossy()])
            .status()
            .context("cp -r failed")?;
    } else {
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn remove_path(path: &std::path::Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}
