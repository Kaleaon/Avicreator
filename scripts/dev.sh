#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
if [[ ! -x .venv/bin/python ]]; then
  echo 'Run bash scripts/setup.sh first.' >&2
  exit 1
fi

command_name=${1:-serve}
if [[ $# -gt 0 ]]; then shift; fi
case "$command_name" in
  test)
    export PYTHONPATH="$repo_root/..:$repo_root${PYTHONPATH:+:$PYTHONPATH}"
    exec .venv/bin/python -m pytest "$@"
    ;;
  serve)
    exec .venv/bin/python -m uvicorn webapp.server:app --host 127.0.0.1 --port 8000 "$@"
    ;;
  *)
    echo 'Usage: bash scripts/dev.sh {test|serve} [arguments...]' >&2
    exit 2
    ;;
esac
