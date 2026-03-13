#!/usr/bin/env bash
# 从整个 Git 历史中移除 frontend/node_modules 和 frontend/.next，缩小仓库体积
# 使用前请先安装: brew install git-filter-repo  或  pip install git-filter-repo
set -e
cd "$(dirname "$0")/.."

if ! command -v git-filter-repo &>/dev/null; then
  echo "请先安装 git-filter-repo:"
  echo "  brew install git-filter-repo"
  echo "  或: pip install git-filter-repo"
  exit 1
fi

echo "将从历史中移除: frontend/node_modules, frontend/.next"
echo "执行后需要 force push（若已 push 过）：git push --force"
read -p "继续? [y/N] " -n 1 -r; echo
[[ $REPLY =~ ^[yY]$ ]] || exit 0

git filter-repo --force \
  --path frontend/node_modules --invert-paths \
  --path frontend/.next --invert-paths

echo "完成。请执行: git push --force"
echo "（若为首次 push 则直接 git push 即可）"
