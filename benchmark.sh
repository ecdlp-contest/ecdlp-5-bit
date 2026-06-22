#!/usr/bin/env bash
# Benchmark a 5-bit Shor ECDLP submission.
#
# This follows the ECDSA Fail point-add convention:
# 1. Wipe stale ops.bin / score.json so a contestant cannot pre-seed them.
# 2. Build trusted binaries from the locked dependency graph.
# 3. Run build_circuit (UNTRUSTED: imports src/shor_oracle) in a read-only,
#    no-network scratch sandbox when bubblewrap or sandbox-exec is available.
# 4. Verify ops.bin exists, then run eval_circuit (TRUSTED: does not import
#    src/shor_oracle) to validate 9024 Fiat-Shamir oracle shots and write score.json/results.tsv.
#
# All command-line arguments are forwarded to eval_circuit, e.g. --note "...".
set -euo pipefail

# shellcheck disable=SC1091
. "$HOME/.cargo/env" 2>/dev/null || true

if [ "$#" -eq 0 ]; then
  set -- --note "cli benchmark"
fi

pinned_rust_channel() {
  local channel=""
  if [[ -f rust-toolchain ]]; then
    channel="$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain | sed -n '1p')"
    if [[ -z "${channel}" ]]; then
      channel="$(sed -n '1s/^[[:space:]]*\([^[:space:]]*\)[[:space:]]*$/\1/p' rust-toolchain)"
    fi
  fi
  printf '%s\n' "${channel}"
}

installed_toolchain_for_channel() {
  local channel="$1" line toolchain toolchains
  [[ -n "${channel}" ]] || return 1
  command -v rustup >/dev/null 2>&1 || return 1
  toolchains="$(rustup toolchain list 2>/dev/null || true)"
  while IFS= read -r line; do
    toolchain="${line%% *}"
    if [[ "${toolchain}" == "${channel}" || "${toolchain}" == "${channel}-"* ]]; then
      printf '%s\n' "${toolchain}"
      return 0
    fi
  done <<< "${toolchains}"
  return 1
}

require_offline_rust_toolchain() {
  if [[ -n "${RUSTUP_TOOLCHAIN:-}" ]] || ! command -v rustup >/dev/null 2>&1; then
    return 0
  fi
  local channel toolchain
  channel="$(pinned_rust_channel)"
  [[ -n "${channel}" ]] || return 0
  toolchain="$(installed_toolchain_for_channel "${channel}" || true)"
  if [[ -z "${toolchain}" ]]; then
    echo "!! pinned Rust toolchain '${channel}' is not installed; run ./setup.sh before offline benchmarking" >&2
    exit 1
  fi
  export RUSTUP_TOOLCHAIN="${toolchain}"
}

find_c_compiler() {
  if [[ -n "${CC:-}" ]] && command -v "${CC}" >/dev/null 2>&1; then
    command -v "${CC}"
    return 0
  fi
  local candidate
  for candidate in gcc cc clang; do
    if command -v "${candidate}" >/dev/null 2>&1; then
      command -v "${candidate}"
      return 0
    fi
  done
  return 1
}

compiler="$(find_c_compiler || true)"
if [[ -z "${compiler}" ]]; then
  echo "!! no C compiler/linker found; run ./setup.sh or install gcc/clang" >&2
  exit 1
fi
export CC="${compiler}"
require_offline_rust_toolchain
if ! command -v cargo >/dev/null 2>&1; then
  echo "!! cargo not found; run ./setup.sh before offline benchmarking" >&2
  exit 1
fi
export CARGO_NET_OFFLINE=true

rm -f ops.bin score.json

target_dir="${CARGO_TARGET_DIR:-target}"
RUSTFLAGS="-C linker=${compiler}" cargo build --release --locked --offline --bin build_circuit --bin eval_circuit
if [[ "${target_dir}" = /* ]]; then
  target_abs="${target_dir}"
else
  target_abs="$(pwd)/${target_dir}"
fi
build_circuit_bin="${target_abs}/release/build_circuit"
eval_circuit_bin="${target_abs}/release/eval_circuit"

ops_scratch="$(cd "$(mktemp -d)" && pwd -P)"
chmod 0777 "${ops_scratch}"
install -m 0755 "${build_circuit_bin}" "${ops_scratch}/build_circuit"
sandbox_build_circuit_bin="${ops_scratch}/build_circuit"
bwrap_via_sudo=0

if command -v bwrap >/dev/null 2>&1; then
  bw=(bwrap)
  if [[ "$(id -u)" -ne 0 ]] && command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
    bw=(sudo -n bwrap)
    bwrap_via_sudo=1
  elif command -v setpriv >/dev/null 2>&1; then
    bw=(setpriv --no-new-privs bwrap)
  fi
  run_build=(
    "${bw[@]}"
    --ro-bind / /
    --dev /dev
    --ro-bind /proc /proc
    --bind "${ops_scratch}" "${ops_scratch}"
    --chdir "${ops_scratch}"
    --setenv TMPDIR "${ops_scratch}"
    --unshare-user
    --unshare-net
    --unshare-ipc
    --unshare-uts
    --unshare-cgroup
    --cap-drop ALL
    --new-session
    --die-with-parent
    --uid 65534
    --gid 65534
    -- "${sandbox_build_circuit_bin}"
  )
elif [[ "$(uname -s)" == "Darwin" ]] && command -v sandbox-exec >/dev/null 2>&1; then
  macos_profile="(version 1)(allow default)(deny file-write*)(allow file-write* (subpath \"${ops_scratch}\"))(allow file-write* (subpath \"/dev\"))(deny network*)"
  run_build=(sandbox-exec -p "${macos_profile}" /bin/bash -c 'cd "$1" && export TMPDIR="$1" && exec "$2"' _ "${ops_scratch}" "${sandbox_build_circuit_bin}")
else
  echo "!! no sandbox available (bubblewrap/sandbox-exec); running build_circuit UNCONFINED (dev fallback)" >&2
  run_build=(bash -c 'cd "$1" && exec "$2"' _ "${ops_scratch}" "${sandbox_build_circuit_bin}")
fi

cleanup_pgid=""
reap() {
  [[ -n "${cleanup_pgid}" ]] || return 0
  if [[ "${bwrap_via_sudo}" -eq 1 ]]; then
    sudo -n kill -KILL -"${cleanup_pgid}" 2>/dev/null || true
  else
    kill -KILL -"${cleanup_pgid}" 2>/dev/null || true
  fi
}
cleanup() {
  reap
  if [[ -n "${ops_scratch:-}" ]]; then
    rm -rf "${ops_scratch}" 2>/dev/null || true
  fi
}
trap cleanup EXIT

if command -v setsid >/dev/null 2>&1; then
  setsid "${run_build[@]}" &
  build_pid=$!
  cleanup_pgid="${build_pid}"
  set +e
  wait "${build_pid}"
  build_status=$?
  set -e
  reap
  cleanup_pgid=""
else
  set -m
  "${run_build[@]}" &
  build_pid=$!
  cleanup_pgid="${build_pid}"
  set +e
  wait "${build_pid}"
  build_status=$?
  set -e
  reap
  cleanup_pgid=""
  set +m
fi

if [[ "${build_status}" -ne 0 ]]; then
  echo "!! build_circuit exited with status ${build_status}" >&2
  exit "${build_status}"
fi

if [[ -s "${ops_scratch}/ops.bin" ]]; then
  cp "${ops_scratch}/ops.bin" ./ops.bin
fi
rm -rf "${ops_scratch}"
ops_scratch=""

if [[ ! -s ops.bin ]]; then
  echo "!! build_circuit did not produce ops.bin" >&2
  exit 1
fi

"${eval_circuit_bin}" "$@"
