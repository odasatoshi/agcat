#!/usr/bin/env bash
# main を手元から直接動かさせない。作業はブランチを切って PR に載せる（CLAUDE.md の開発フロー参照）。
# 判定できないときは通さない側に倒す。手元の防壁はこれ 1 枚なので、fail open にすると意味がない。
set -euo pipefail

deny() {
  echo "$1 ブランチを切って PR 経由で入れること（CLAUDE.md の開発フロー参照）。" >&2
  exit 2
}

command -v jq >/dev/null 2>&1 || deny "jq がないので main への書き込みか判定できない。"
cmd=$(jq -r '.tool_input.command // ""') || deny "hook の入力を解釈できない。"

# 改行と連続空白を畳む。git -C や git -c、行継続で判定をすり抜けられないようにする。
flat=$(printf '%s' "$cmd" | tr '\n\t' '  ' | tr -s ' ')

# main を進める書き込み。commit と push だけ見ても、merge や reset で main は動く。
writes='commit|push|merge|rebase|cherry-pick|revert|am|reset|update-ref'
grep -Eq "(^|[^[:alnum:]_-])git( |$)" <<< "$flat" || exit 0
grep -Eq "(^|[^[:alnum:]_-])($writes)( |$)" <<< "$flat" || exit 0
# ここから先は git の書き込み系コマンド。無害なコマンドが文字列として "git commit" を
# 含む場合も止まるが、main の上だけの話なので通す側には倒さない。

# ブランチにいても、refspec で main を書き換える push は素通りさせない。
if grep -Eq "(^|[^[:alnum:]_-])push( |$)" <<< "$flat" \
  && grep -Eq "(^| |:)\+?(main|refs/heads/main)( |$)" <<< "$flat"; then
  deny "main を対象にした push はしない。"
fi

# 実行前の HEAD を見るだけでは、同じコマンドで main に切り替えてから書く経路を防げない。
if grep -Eq "(^|[^[:alnum:]_-])(switch|checkout)( +-[^ ]+)* +main( |$)" <<< "$flat"; then
  deny "main に切り替えてから書き込むコマンドはしない。"
fi

[ "$(git symbolic-ref --quiet --short HEAD 2>/dev/null || true)" = main ] || exit 0
deny "main の上で git の書き込みはしない。"
