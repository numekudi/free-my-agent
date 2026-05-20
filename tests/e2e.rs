use anyhow::{Context, Result, anyhow, ensure};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_free-my-agent");

struct TestRepo {
    repo: TempDir,
    home: TempDir,
}

impl TestRepo {
    fn new() -> Result<Self> {
        let repo = TempDir::new().context("failed to create temp repo dir")?;
        let home = TempDir::new().context("failed to create temp home dir")?;
        let instance = Self { repo, home };

        instance.git_ok(&["init"])?;
        instance.git_ok(&["config", "user.name", "free-my-agent tests"])?;
        instance.git_ok(&["config", "user.email", "free-my-agent@example.com"])?;

        Ok(instance)
    }

    fn path(&self) -> &Path {
        self.repo.path()
    }

    fn git_dir(&self) -> PathBuf {
        self.path().join(".git")
    }

    fn backup_root(&self) -> PathBuf {
        self.home.path().join(".config/free-my-agent/backup")
    }

    fn backup_repo_dir(&self) -> Result<PathBuf> {
        let mut entries = fs::read_dir(self.backup_root())
            .context("failed to read backup root")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect backup dirs")?;
        ensure!(entries.len() == 1, "expected exactly one backup directory");
        Ok(entries.remove(0).path())
    }

    fn run(&self, args: &[&str]) -> Result<Output> {
        Command::new(BIN)
            .args(args)
            .current_dir(self.path())
            .env("HOME", self.home.path())
            .output()
            .with_context(|| format!("failed to run {BIN} {}", args.join(" ")))
    }

    fn run_ok(&self, args: &[&str]) -> Result<Output> {
        let output = self.run(args)?;
        ensure_success(output, &format!("free-my-agent {}", args.join(" ")))
    }

    fn git(&self, args: &[&str]) -> Result<Output> {
        Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("HOME", self.home.path())
            .output()
            .with_context(|| format!("failed to run git {}", args.join(" ")))
    }

    fn git_ok(&self, args: &[&str]) -> Result<Output> {
        let output = self.git(args)?;
        ensure_success(output, &format!("git {}", args.join(" ")))
    }

    fn write_file(&self, rel: &str, content: &str) -> Result<()> {
        let path = self.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))
    }

    fn read_file(&self, rel: &str) -> Result<String> {
        let path = self.path().join(rel);
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))
    }

    fn exists(&self, rel: &str) -> bool {
        self.path().join(rel).exists()
    }
}

fn ensure_success(output: Output, command: &str) -> Result<Output> {
    if output.status.success() {
        return Ok(output);
    }

    Err(anyhow!(
        "{command} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

#[test]
fn init_installs_hooks_and_default_patterns() -> Result<()> {
    let repo = TestRepo::new()?;

    repo.run_ok(&["init"])?;

    let pre_commit = fs::read_to_string(repo.git_dir().join("hooks/pre-commit"))
        .context("failed to read pre-commit hook")?;
    let post_commit = fs::read_to_string(repo.git_dir().join("hooks/post-commit"))
        .context("failed to read post-commit hook")?;
    let managed = fs::read_to_string(repo.git_dir().join("free-my-agent"))
        .context("failed to read managed patterns file")?;

    assert!(pre_commit.contains("# free-my-agent:"));
    assert!(pre_commit.contains(" restore"));
    assert!(post_commit.contains("# free-my-agent:"));
    assert!(post_commit.contains(" free"));
    assert!(managed.contains("CLAUDE.md"));
    assert!(managed.contains(".claude"));
    assert!(managed.contains(".github/prompts/*"));

    Ok(())
}

#[test]
fn free_and_restore_round_trip_managed_files() -> Result<()> {
    let repo = TestRepo::new()?;
    repo.write_file("CLAUDE.md", "system prompt\n")?;
    repo.write_file(".claude/settings.toml", "model = \"sonnet\"\n")?;

    repo.run_ok(&["init"])?;
    repo.run_ok(&["free"])?;

    assert!(!repo.exists("CLAUDE.md"));
    assert!(!repo.exists(".claude"));

    let backup = repo.backup_repo_dir()?;
    assert!(backup.join("CLAUDE.md").exists());
    assert!(backup.join(".claude").exists());

    repo.run_ok(&["restore"])?;

    assert_eq!(repo.read_file("CLAUDE.md")?, "system prompt\n");
    assert_eq!(
        repo.read_file(".claude/settings.toml")?,
        "model = \"sonnet\"\n"
    );

    Ok(())
}

#[test]
fn uninit_restores_hidden_files_and_removes_hooks() -> Result<()> {
    let repo = TestRepo::new()?;
    repo.write_file("CLAUDE.md", "restored on uninit\n")?;
    repo.write_file(".claude/agents.json", "{\"enabled\":true}\n")?;

    repo.run_ok(&["init"])?;
    repo.run_ok(&["free"])?;
    assert!(!repo.exists("CLAUDE.md"));
    assert!(!repo.exists(".claude"));

    repo.run_ok(&["uninit"])?;

    assert_eq!(repo.read_file("CLAUDE.md")?, "restored on uninit\n");
    assert_eq!(
        repo.read_file(".claude/agents.json")?,
        "{\"enabled\":true}\n"
    );
    assert!(!repo.git_dir().join("hooks/pre-commit").exists());
    assert!(!repo.git_dir().join("hooks/post-commit").exists());

    Ok(())
}

#[test]
fn git_commit_hooks_restore_for_commit_and_hide_afterward() -> Result<()> {
    let repo = TestRepo::new()?;
    repo.write_file("CLAUDE.md", "commit-visible\n")?;
    repo.write_file(".claude/settings.toml", "hook = true\n")?;
    repo.write_file("note.txt", "normal file\n")?;

    repo.run_ok(&["init"])?;
    repo.run_ok(&["free"])?;
    repo.git_ok(&["add", "note.txt"])?;

    repo.git_ok(&["commit", "-m", "exercise hooks"])?;

    assert!(!repo.exists("CLAUDE.md"));
    assert!(!repo.exists(".claude"));

    let tracked = String::from_utf8(
        repo.git_ok(&["ls-tree", "-r", "--name-only", "HEAD"])?
            .stdout,
    )
    .context("git ls-tree output was not utf-8")?;
    assert!(tracked.lines().any(|line| line == "CLAUDE.md"));
    assert!(tracked.lines().any(|line| line == ".claude/settings.toml"));
    assert!(tracked.lines().any(|line| line == "note.txt"));

    let status = String::from_utf8(repo.git_ok(&["status", "--short"])?.stdout)
        .context("git status output was not utf-8")?;
    assert!(status.contains(" D CLAUDE.md"));
    assert!(status.contains(" D .claude/settings.toml"));

    Ok(())
}
