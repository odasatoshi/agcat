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

- main への直接 push、force push、ブランチの削除は ruleset が拒否する。bypass actor は置いていないので、admin でも PR を経由する。
- `ci` チェック（fmt / clippy / test）と `gates` チェック（issue の紐付け、レビューのラベル）が緑でなければ merge できない。`gates` はイベントのペイロードではなく API の現況を読むので、レビュー後に push すればラベルが外れて赤に戻る。
- 手元では `.claude/hooks/guard-main.sh` が、main の上での git の書き込み（commit、push、merge、rebase、cherry-pick、revert、am、reset、update-ref）と、ブランチから `main` を対象に push する形、同じコマンドで main に切り替えてから書く形を止める。入力を解釈できないときも止める。

bypass actor を置かない代わりに、どうしても外す必要があるときは Settings > Rules で ruleset 自体を一時的に無効化する（無効化は履歴に残る）。

## コードの決まり

- コメントと commit message は日本語で書く。commit message の一行目は「〜する」で終える一文にする。
- なぜそう書いたかがコードから読み取れない箇所にだけコメントを置く。
- プレビューは先頭 64 KiB（`PREVIEW_LIMIT`）しか読まない。大きいファイルで固まらせないための制約なので、ファイル全体を読む実装に戻さない。
