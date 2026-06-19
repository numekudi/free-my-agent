use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{backup_dir, repo_root, resolve_targets};

struct BackupEntryMatcher {
    patterns: Vec<glob::Pattern>,
}

impl BackupEntryMatcher {
    fn new(patterns: &[String]) -> Result<Self> {
        let patterns = patterns
            .iter()
            .map(|pattern| {
                glob::Pattern::new(pattern)
                    .with_context(|| format!("invalid glob pattern: {pattern}"))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { patterns })
    }

    fn matches(&self, path: &Path) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern.matches_path(path))
    }
}

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
            .with_context(|| format!("{} is outside {}", target.display(), root.display()))?;
        let dest = backup.join(rel);
        copy_to(target, &dest).with_context(|| format!("failed to backup {}", target.display()))?;
        remove_path(target).with_context(|| format!("failed to remove {}", target.display()))?;
        println!("freed: {}", rel.display());
    }
    Ok(())
}

pub fn restore() -> Result<()> {
    let backup = backup_dir()?;
    let root = repo_root()?;
    let patterns = crate::config::read_patterns()?;
    let matcher = BackupEntryMatcher::new(&patterns)?;

    for rel in matching_backup_entries(&backup, &matcher)? {
        let src = backup.join(&rel);
        let dest = root.join(&rel);
        copy_to(&src, &dest).with_context(|| format!("failed to restore {}", rel.display()))?;
        remove_path(&src).with_context(|| format!("failed to clear backup {}", rel.display()))?;

        let output = Command::new("git")
            .args(["add", &dest.to_string_lossy()])
            .current_dir(&root)
            .output()
            .with_context(|| format!("failed to git add {}", rel.display()))?;
        anyhow::ensure!(
            output.status.success(),
            "git add failed for {}\nstdout:\n{}\nstderr:\n{}",
            rel.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        println!("restored: {}", rel.display());
    }
    Ok(())
}

pub fn status() -> Result<()> {
    let backup = backup_dir()?;
    let patterns = crate::config::read_patterns()?;
    let matcher = BackupEntryMatcher::new(&patterns)?;
    let entries = matching_backup_entries(&backup, &matcher)?;

    if entries.is_empty() {
        println!("no files currently hidden");
        return Ok(());
    }

    for rel in entries {
        let path = backup.join(&rel);
        let label = if path.is_dir() {
            "hidden (dir)"
        } else {
            "hidden"
        };
        println!("{label}: {}", rel.display());
    }

    Ok(())
}

fn matching_backup_entries(backup: &Path, matcher: &BackupEntryMatcher) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    collect_matching_backup_entries(backup, backup, matcher, &mut entries)?;
    entries.sort();
    Ok(entries)
}

fn collect_matching_backup_entries(
    backup: &Path,
    dir: &Path,
    matcher: &BackupEntryMatcher,
    entries: &mut Vec<PathBuf>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path
            .strip_prefix(backup)
            .with_context(|| format!("{} is outside {}", path.display(), backup.display()))?
            .to_path_buf();

        if matcher.matches(&rel) {
            entries.push(rel);
            continue;
        }

        if path.is_dir() {
            collect_matching_backup_entries(backup, &path, matcher, entries)?;
        }
    }

    Ok(())
}

fn copy_to(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        if dst.exists() {
            remove_path(dst)?;
        }
        copy_dir(src, dst)?;
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        if dst.is_dir() {
            fs::remove_dir_all(dst)?;
        }
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    // Preserve directory structure exactly so restore can rehydrate paths from
    // the backup root without flattening nested glob matches.
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
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
