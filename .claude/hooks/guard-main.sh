#!/usr/bin/env bash
# main を手元から直接動かさせない。作業はブランチを切って PR に載せる（CLAUDE.md の開発フロー参照）。
# 判定できないときは通さない側に倒す。ただし git と無関係なコマンドは巻き込まない。
#
# 拾えない形：`cd <別のクローン> && git commit`。cwd と -C の指すリポジトリしか見ていない。
# remote の main はサーバ側の ruleset が守るので、この hook は素直な形の取りこぼしを拾う一次防壁である。
set -euo pipefail

deny() {
  echo "$1 ブランチを切って PR 経由で入れること（CLAUDE.md の開発フロー参照）。" >&2
  exit 2
}

raw=$(cat)
if ! command -v jq >/dev/null 2>&1; then
  # jq がなくても、git に触らないコマンドを巻き込む理由はない。
  grep -q git <<< "$raw" && deny "jq がないので main への書き込みか判定できない。"
  exit 0
fi
cmd=$(jq -r '.tool_input.command // ""' <<< "$raw") || deny "hook の入力を解釈できない。"

# 引用符とシェルの区切り子を空白に潰す。"HEAD:main" や main&&git のような書き方で
# トークン境界の判定を外されないようにする。
flat=$(printf '%s' "$cmd" | tr '\n\t;&|()"'"'" '         ' | tr -s ' ')

m() { grep -Eq "$1" <<< "$flat"; }

# git のグローバルオプション（-C dir, -c k=v, --git-dir=... など）を読み飛ばしてサブコマンドを見る。
G='(^| )git( +(-[Cc] +[^ ]+|--[a-z-]+(=[^ ]+)?|-[a-zA-Z]+))*'
WRITES='commit|push|merge|rebase|cherry-pick|revert|am|reset|update-ref'

# git を触らないコマンドは、そもそも関係ない。地の文の "am" などに反応させない。
m "(^| )git( |$)" || exit 0

# main の ref を書き換える形は、どのブランチにいても止める。書き込み動詞に現れない
# fetch の refspec や branch -f も、main を動かす経路である。
if m "( |:)refs/heads/main( |$)|:main( |$)"; then
  deny "main の ref を直接書き換える操作はしない。"
fi
if m "$G +push( |$)" && m "( )\\+?main( |$)|( )(--all|--mirror)( |$)"; then
  deny "main を対象にした push はしない。"
fi
if m "$G +branch +((-[a-zA-Z]+|--force|--move|--delete) +)*main( |$)"; then
  deny "git branch で main を付け替える操作はしない。"
fi
# 実行前の HEAD を見るだけでは、同じコマンドで main に切り替えてから書く経路を防げない。
if m "$G +(switch|checkout)( +-[a-zA-Z]+)* +main( |$)" && m "$G +($WRITES)( |$)"; then
  deny "main に切り替えてから書き込むコマンドはしない。"
fi

# ここから先は、main の上での書き込みを見る。
m "$G +($WRITES)( |$)" || exit 0

# cwd と、-C が指すリポジトリの HEAD を見る。
head_of() { git -C "${1:-.}" symbolic-ref --quiet --short HEAD 2>/dev/null || true; }
on_main=no
[ "$(head_of)" = main ] && on_main=yes
dir=$(grep -Eo '(^| )git +-C +[^ ]+' <<< "$flat" | head -1 | awk '{print $NF}' || true)
if [ -n "$dir" ] && [ "$(head_of "$dir")" = main ]; then on_main=yes; fi
[ "$on_main" = yes ] || exit 0

# main の上でも、新しいブランチへ移ってから書く形は通す。hook 自身が案内している操作なので。
if m "$G +(switch +-c|checkout +-b) +[^ ]+" && ! m "$G +(switch +-c|checkout +-b) +main( |$)"; then
  exit 0
fi
deny "main の上で git の書き込みはしない。"
