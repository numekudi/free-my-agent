# free-my-agent

作業中だけ `CLAUDE.md` や `AGENTS.md` などのエージェント指示ファイルを隠し、`git commit` 時に自動復元するツール。他人のリポジトリを汚さず、自分の指示だけでエージェントを動かせる。

## インストール

```sh
cargo install --git https://github.com/numekudi/free-my-agent
```

または手元でビルド:

```sh
git clone https://github.com/numekudi/free-my-agent
cd free-my-agent
cargo install --path .
```

## セットアップ

```sh
# 使いたいリポジトリで
free-my-agent init   # git hooks をインストール
free-my-agent free   # 対象ファイルを隠してエージェントを解放
```

以降は `git commit` のたびにファイルが自動復元→コミット→再隠蔽される。

## 使い方

```
$ free-my-agent --help

Hide agent instruction files during work, restore them on commit

Usage: free-my-agent <COMMAND>

Commands:
  init     Install git hooks into .git/hooks/
  uninit   Remove git hooks installed by init
  add      Add a glob pattern to managed list (default: local to this repo)
  remove   Remove a glob pattern from managed list (default: local to this repo)
  list     List managed patterns
  free     Backup and delete managed files (free the agent)
  restore  Restore managed files from backup (called by pre-commit hook)
  status   Show which files are currently hidden
  help     Print this message or the help of the given subcommand(s)
```

パターンの管理はデフォルトでリポジトリローカル (`.git/free-my-agent`)、`--global` で全リポジトリ共通 (`~/.config/free-my-agent/managed`) に保存される。

```sh
free-my-agent add .cursor          # このリポジトリだけに追加
free-my-agent add --global GEMINI.md  # 全リポジトリに追加
free-my-agent list                 # 登録済みパターンを確認
```

### デフォルトパターン

`init` を実行すると `.git/free-my-agent` に以下が自動生成される:

```
CLAUDE.md
AGENTS.md
.claude
.gemini
.agents
.github/copilot-instructions.md
.github/instructions/*
.github/prompts/*
.github/skills/*
```

このファイルを直接編集するか、`add` / `remove` コマンドで変更できる。
