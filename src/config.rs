use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternScope {
    Global,
    Local,
}

impl PatternScope {
    pub fn from_global_flag(global: bool) -> Self {
        if global { Self::Global } else { Self::Local }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Local => "local",
        }
    }
}

pub fn repo_root() -> Result<PathBuf> {
    git_path(["rev-parse", "--show-toplevel"])
}

pub fn repo_id() -> Result<String> {
    let root = repo_root()?;
    let mut hasher = Sha256::new();
    hasher.update(root.to_string_lossy().as_bytes());
    let hash = hasher.finalize();
    Ok(hex::encode(&hash[..4]))
}

pub fn config_base() -> Result<PathBuf> {
    let base = user_config_dir()?;
    fs::create_dir_all(&base)?;
    Ok(base)
}

fn user_config_dir() -> Result<PathBuf> {
    // The CLI is user-scoped; a missing HOME is an environment error, not a
    // condition to silently redirect into another user's config directory.
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".config").join("free-my-agent"))
}

pub fn backup_dir() -> Result<PathBuf> {
    let id = repo_id()?;
    let dir = config_base()?.join("backup").join(id);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn global_managed_file() -> Result<PathBuf> {
    let path = config_base()?.join("managed");
    if !path.exists() {
        fs::write(&path, "")?;
    }
    Ok(path)
}

pub fn local_managed_file() -> Result<PathBuf> {
    let git_dir = git_dir()?;
    Ok(git_dir.join("free-my-agent"))
}

fn git_dir() -> Result<PathBuf> {
    git_path(["rev-parse", "--git-dir"])
}

fn git_path<const N: usize>(args: [&str; N]) -> Result<PathBuf> {
    let out = Command::new("git")
        .args(args)
        .output()
        .context("failed to run git")?;
    anyhow::ensure!(out.status.success(), "not inside a git repository");
    let path = String::from_utf8(out.stdout)?.trim().to_string();
    Ok(PathBuf::from(path))
}

fn parse_file(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(path)?;
    Ok(content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect())
}

pub fn read_local_patterns() -> Result<Vec<String>> {
    parse_file(&local_managed_file()?)
}

pub fn read_global_patterns() -> Result<Vec<String>> {
    parse_file(&global_managed_file()?)
}

pub fn read_patterns() -> Result<Vec<String>> {
    // BTreeSet gives deterministic output while removing duplicated patterns
    // across local and global config files.
    let mut patterns = BTreeSet::new();
    patterns.extend(read_global_patterns()?);
    patterns.extend(read_local_patterns()?);
    Ok(patterns.into_iter().collect())
}

fn append_pattern(path: &Path, pattern: &str) -> Result<()> {
    let mut content = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    content.push_str(pattern);
    content.push('\n');
    fs::write(path, content)?;
    Ok(())
}

pub fn add_pattern(pattern: &str, global: bool) -> Result<()> {
    let scope = PatternScope::from_global_flag(global);
    let path = managed_file(scope)?;
    let existing = parse_file(&path)?;
    if existing.iter().any(|p| p == pattern) {
        println!("already registered: {pattern}");
        return Ok(());
    }
    append_pattern(&path, pattern)?;
    println!("added ({}): {pattern}", scope.label());
    Ok(())
}

pub fn remove_pattern(pattern: &str, global: bool) -> Result<()> {
    let scope = PatternScope::from_global_flag(global);
    let path = managed_file(scope)?;
    if !path.exists() {
        println!("pattern not found: {pattern}");
        return Ok(());
    }
    let content = fs::read_to_string(&path)?;
    let new_content: String = content
        .lines()
        .filter(|l| l.trim() != pattern)
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(&path, new_content)?;
    println!("removed ({}): {pattern}", scope.label());
    Ok(())
}

fn managed_file(scope: PatternScope) -> Result<PathBuf> {
    match scope {
        PatternScope::Global => global_managed_file(),
        PatternScope::Local => local_managed_file(),
    }
}

pub fn resolve_targets() -> Result<Vec<PathBuf>> {
    let root = repo_root()?;
    let patterns = read_patterns()?;
    let mut targets = BTreeSet::new();
    for pattern in &patterns {
        let full_pattern = root.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();
        for entry in
            glob::glob(&pattern_str).with_context(|| format!("invalid glob pattern: {pattern}"))?
        {
            let path = entry?;
            if path.is_file() || path.is_dir() {
                targets.insert(path);
            }
        }
    }
    Ok(targets.into_iter().collect())
}
