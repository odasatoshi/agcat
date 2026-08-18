# agcat

ターミナル上でディレクトリを辿り、ファイルの中身をプレビューする TUI。
実装は `src/main.rs` の一枚で、外部クレートは ratatui だけに留めている。

## 開発フロー

変更は次の順序で進める。main を直接動かさない。

1. **issue 登録**：症状・原因・直し方を書く。計測できるものは数値を添える。
2. **開発**：`git switch -c <種別>/<短い説明>` でブランチを切る（例 `fix/wrapped-line-cache`）。
3. **試験**：`cargo test`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` を通す。振る舞いの変更にはテストを足す。
4. **Code Review**：`/code-review` を実行し、指摘を PR にコメントとして残す。
5. **PR**：本文に `Closes #<issue 番号>` を必ず書く。書き忘れると gates チェックが落ちる。
6. **Reality Check**：Reality Checker エージェントで、主張と実際の動作が合っているかを確かめ、結果を PR にコメントする。
7. **merge**：4 と 6 を終えたら `reviewed` と `reality-checked` ラベルを付ける。両方揃い CI が緑になると merge できる。squash merge で main に入り、ブランチは自動で消える。

新しいコミットを push すると、gates ワークフローが 2 つのラベルを外す。
レビューは最後のコミットに対してやり直す。

## 仕組みで止まること

- main への直接 push と force push は ruleset が拒否する。bypass はない。
- `ci` チェック（fmt / clippy / test）と `gates` チェック（issue 紐付け、レビューのラベル）が緑でなければ merge できない。
- 手元では、main の上での `git commit` / `git push` を `.claude/hooks/guard-main.sh` が止める。

緊急時にフローを外すには、GitHub の Settings > Rules から ruleset を一時的に無効化する。

## コードの決まり

- コメントと commit message は日本語で書く。commit message の一行目は「〜する」で終える一文にする。
- なぜそう書いたかがコードから読み取れない箇所にだけコメントを置く。
- プレビューは先頭 64 KiB（`PREVIEW_LIMIT`）しか読まない。大きいファイルで固まらせないための制約なので、ファイル全体を読む実装に戻さない。
