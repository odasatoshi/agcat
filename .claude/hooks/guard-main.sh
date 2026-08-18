#!/usr/bin/env bash
# main の上で commit / push させない。作業はブランチを切って PR にする（CLAUDE.md の開発フロー参照）。
cmd=$(jq -r '.tool_input.command // ""')
case "$cmd" in
  *"git commit"*|*"git push"*) ;;
  *) exit 0 ;;
esac
[ "$(git symbolic-ref --quiet --short HEAD 2>/dev/null)" = main ] || exit 0
echo "main では commit / push しない。git switch -c <branch> でブランチを切り、PR 経由で入れること（CLAUDE.md の開発フロー参照）。" >&2
exit 2
