# agcat

ターミナル上でディレクトリを辿り、ファイルの中身をプレビューする TUI。
実装は `src/main.rs` の一枚で、外部クレートは ratatui だけに留めている。

## 開発フロー

変更は次の順序で進める。main を直接動かさない。

1. **issue 登録**：症状・原因・直し方を書く。計測できるものは数値を添える。
2. **開発**：`git switch -c <種別>/<短い説明>` でブランチを切る（例 `fix/wrapped-line-cache`）。
3. **試験**：`cargo test`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`、`bash .claude/hooks/guard-main.test.sh` を通す。振る舞いの変更にはテストを足す。
4. **Code Review**：`/code-review` を実行し、指摘を PR にコメントとして残す。
5. **PR**：本文に `Closes #<issue 番号>` を必ず書く。書き忘れると gates チェックが落ちる。
6. **Reality Check**：Reality Checker エージェントで、主張と実際の動作が合っているかを確かめ、結果を PR にコメントする。
7. **merge**：4 と 6 を終えたら `reviewed` と `reality-checked` ラベルを付ける。両方揃い CI が緑になると merge できる。squash merge で main に入り、ブランチは自動で消える。

新しいコミットを push すると、gates ワークフローが 2 つのラベルを外す。
レビューは最後のコミットに対してやり直す。

## 仕組みで止まること

- main への直接 push、force push、main の削除は ruleset が拒否する。bypass actor は置いていないので、admin でも PR を経由する。
- `ci` チェック（fmt / clippy / test / hook の照合表）と `gates` チェック（issue の紐付け、レビューのラベル）が緑でなければ merge できない。
- `gates` は判定をイベントのペイロードではなく API の現況から読む。レビュー後に push すればラベルが外れ、赤に戻る。
- `gates` は `pull_request_target` で走るので、判定に使われる定義は base（main）側のものである。`gates.yml` を無害化した版が自分の判定に使われることはない。
- PR に未解決のレビュースレッドがあると merge できない（ruleset の `required_review_thread_resolution`）。指摘は直すか、返答して解決にしてから merge する。
- 手元では `.claude/hooks/guard-main.sh` が、main の上での git の書き込み（commit、push、merge、rebase、cherry-pick、revert、am、reset、update-ref）、ブランチから `main` を対象にする push や ref の書き換え、同じコマンドで main に切り替えてから書く形を止める。入力を解釈できないときも止める。何を止め何を通すかは `.claude/hooks/guard-main.test.sh` に照合表として書いてあり、CI で回している。

hook は cwd と `git -C` の指すリポジトリしか見ないので、`cd <別のクローン> && git commit` は拾えない。
手元の main が汚れることは防ぎきれず、remote の main は ruleset が守る、という二段構えだと理解しておく。

## 担保の届く範囲

上の一覧は、忘却と不注意に対しては効くが、write 権限を持つ主体が意図して迂回する場合の境界にはならない。
必須チェックは check-run の名前で照合されるので、job 名 `gates` を持つワークフローを 1 つ足せば同名の緑を作れてしまい、同名の check-run は最後のものが評価される。
`ci` は head のコードを試験する必要があるため `pull_request` で走り、定義は PR の head 側から読まれる。
ラベルも「誰が付けたか」を見ておらず、`Closes #<番号>` も紐付けた issue が変更と関係あるかを見ていない。

つまりこの仕組みが担保するのは「手順を飛ばしたことに気付かないまま merge してしまう」ことの防止である。
意図的に迂回できるかは別の問題で、それを止めるのは ruleset の bypass 不在と、この文書を読む側の規律である。

bypass actor を置かない代わりに、どうしても外す必要があるときは Settings > Rules で ruleset 自体を一時的に無効化する（無効化は履歴に残る）。

## コードの決まり

- コメントと commit message は日本語で書く。commit message の一行目は「〜する」で終える一文にする。
- なぜそう書いたかがコードから読み取れない箇所にだけコメントを置く。
- Rust の版は `rust-toolchain.toml` で固定している。上げるときは、それ自体を 1 つの変更として PR に載せる。
- プレビューは先頭 64 KiB（`PREVIEW_LIMIT`）しか読まない。大きいファイルで固まらせないための制約なので、ファイル全体を読む実装に戻さない。
