#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
jar_path="${TLA2TOOLS_JAR:-${repo_root}/.workspace/tools/tla2tools.jar}"

if [[ ! -f "${jar_path}" ]]; then
  echo "tla2tools.jar not found at ${jar_path}" >&2
  echo "Download it with:" >&2
  echo "  mkdir -p .workspace/tools" >&2
  echo "  curl -L --fail -o .workspace/tools/tla2tools.jar https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar" >&2
  exit 1
fi

java -XX:+UseParallelGC -cp "${jar_path}" tlc2.TLC "$@"
