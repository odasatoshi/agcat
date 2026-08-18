#!/usr/bin/env bash
# guard-main.sh の照合表。使い捨てのリポジトリを作り、hook の標準入力に実際の JSON を流す。
# 塞ぎ方を変えたときに、通してはいけない形と通すべき形の両方が崩れていないかを見る。
set -uo pipefail

hook=$(cd "$(dirname "$0")" && pwd)/guard-main.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

for d in onmain otherclone; do
  git init -qb main "$tmp/$d" && git -C "$tmp/$d" commit -q --allow-empty -m init
done
git init -qb main "$tmp/onbranch" && git -C "$tmp/onbranch" commit -q --allow-empty -m init
git -C "$tmp/onbranch" switch -qc feature

fail=0
check() { # check <ディレクトリ> <期待する終了コード> <コマンド>...
  local dir=$1 want=$2 c rc; shift 2
  for c in "$@"; do
    (cd "$tmp/$dir" && printf '{"tool_input":{"command":%s}}' "$(jq -Rs . <<< "$c")" | "$hook" >/dev/null 2>&1)
    rc=$?
    if [ "$rc" != "$want" ]; then printf 'NG  rc=%s (期待 %s)  %s\n' "$rc" "$want" "$c"; fail=1; fi
  done
}

# main の上での書き込みは止める。git のグローバルオプションや連続空白で外せない。
check onmain 2 'git commit -m x' 'git push' 'git push --force origin main' 'git commit --amend' \
  'git -C . commit -m x' 'git -c user.name=x commit -m y' 'git  commit  -m x' 'git merge feature' \
  'git rebase feature' 'git cherry-pick abc' 'git revert HEAD' 'git am patch' 'git reset --hard abc' \
  'git update-ref refs/heads/main abc'

# main の上でも、読むだけのコマンドと、ブランチを切ってから書く形は通す。
check onmain 0 'git status' 'git log --oneline -3' 'cargo test' 'ls -la' 'git grep reset' \
  'git log --grep commit' 'git status # I am done' 'echo I am here && git status' 'grep -rn "commit" src/' \
  'git switch -c fix/foo && git commit -m x' 'git checkout -b fix/foo && git commit -m x' 'git branch --list main'

# ブランチにいても、main を対象にする形は止める。引用符やシェルの区切り子で外せない。
check onbranch 2 'git push origin HEAD:main' 'git push origin feature:main' 'git push --force origin +HEAD:main' \
  'git push origin main' 'git push origin HEAD:refs/heads/main' 'git push origin "HEAD:main"' "git push origin 'main'" \
  'git push origin HEAD:main;' 'git switch main; git commit -m x' 'git switch main&&git commit -m x' \
  'git checkout main;git commit -m x' 'git update-ref refs/heads/main HEAD' 'git branch -f main HEAD' \
  'git branch --force main HEAD' 'git fetch origin main:main' 'git push origin --all' 'git push --mirror origin'
check onbranch 2 "git -C $tmp/otherclone commit -m x"

# ブランチ上の通常の作業は止めない。
check onbranch 0 'git commit -m x' 'git push' 'git push -u origin feature' 'git push origin feature' \
  'git push -u origin HEAD' 'git merge origin/main' 'git rebase origin/main' 'git switch -c fix/foo' \
  'git fetch origin main' 'git switch main' 'git log origin/main --oneline' 'git push origin maintenance'

# 入力を解釈できないときは通さない。ただし git に触らないコマンドは巻き込まない。
rc=0; (cd "$tmp/onmain" && printf '壊れた JSON' | "$hook" >/dev/null 2>&1) || rc=$?
[ "$rc" = 2 ] || { echo "NG  壊れた入力を通した (rc=$rc)"; fail=1; }
rc=0; (cd "$tmp/onmain" && printf '{"tool_input":{"command":"git commit -m x"}}' | env -i PATH=/bin bash -c "$hook" >/dev/null 2>&1) || rc=$?
[ "$rc" = 2 ] || { echo "NG  jq 不在で git commit を通した (rc=$rc)"; fail=1; }
rc=0; (cd "$tmp/onmain" && printf '{"tool_input":{"command":"ls -la"}}' | env -i PATH=/bin bash -c "$hook" >/dev/null 2>&1) || rc=$?
[ "$rc" = 0 ] || { echo "NG  jq 不在で無関係なコマンドを止めた (rc=$rc)"; fail=1; }

[ "$fail" = 0 ] && echo "guard-main.sh: 照合表をすべて満たした"
exit $fail
