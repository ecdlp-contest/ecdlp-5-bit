#!/usr/bin/env bash
# Ensure Rust, a C linker, and the benchmark sandbox helper are available.
# Idempotent: no-op if the toolchain and cache are already populated.
set -euo pipefail

SUDO=""
if [[ ${EUID:-$(id -u)} -ne 0 ]] && command -v sudo >/dev/null 2>&1; then
  SUDO="sudo"
fi

# shellcheck disable=SC1091
. "$HOME/.cargo/env" 2>/dev/null || true

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

install_system_deps() {
  if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    ${SUDO} apt-get update
    ${SUDO} apt-get install -y --no-install-recommends gcc libc6-dev ca-certificates
  elif command -v dnf >/dev/null 2>&1; then
    ${SUDO} dnf install -y gcc glibc-devel ca-certificates
  elif command -v yum >/dev/null 2>&1; then
    ${SUDO} yum install -y gcc glibc-devel ca-certificates
  elif command -v apk >/dev/null 2>&1; then
    ${SUDO} apk add --no-cache gcc musl-dev ca-certificates
  elif command -v pacman >/dev/null 2>&1; then
    ${SUDO} pacman -Sy --noconfirm gcc ca-certificates
  elif command -v zypper >/dev/null 2>&1; then
    ${SUDO} zypper --non-interactive install gcc glibc-devel ca-certificates
  elif command -v brew >/dev/null 2>&1; then
    :
  else
    return 1
  fi
}

install_cap_tools() {
  if command -v getcap >/dev/null 2>&1 && command -v setcap >/dev/null 2>&1; then
    return 0
  fi
  if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    ${SUDO} apt-get update && ${SUDO} apt-get install -y --no-install-recommends libcap2-bin || true
  elif command -v dnf >/dev/null 2>&1; then
    ${SUDO} dnf install -y libcap || true
  elif command -v yum >/dev/null 2>&1; then
    ${SUDO} yum install -y libcap || true
  elif command -v apk >/dev/null 2>&1; then
    ${SUDO} apk add --no-cache libcap || true
  elif command -v pacman >/dev/null 2>&1; then
    ${SUDO} pacman -Sy --noconfirm libcap || true
  elif command -v zypper >/dev/null 2>&1; then
    ${SUDO} zypper --non-interactive install libcap-progs || true
  fi
}

fix_bwrap_file_caps() {
  local bwrap_path caps
  bwrap_path="$(command -v bwrap 2>/dev/null || true)"
  [[ -n "${bwrap_path}" ]] || return 0
  if ! command -v getcap >/dev/null 2>&1 || ! command -v setcap >/dev/null 2>&1; then
    echo "setup.sh: warning: cannot inspect/repair bwrap capabilities; install libcap tooling" >&2
    return 0
  fi
  caps="$(getcap "${bwrap_path}" 2>/dev/null || true)"
  if [[ -n "${caps}" && ! -u "${bwrap_path}" ]]; then
    echo "setup.sh: removing unsupported file capabilities from ${bwrap_path}" >&2
    if ! ${SUDO} setcap -r "${bwrap_path}"; then
      echo "setup.sh: warning: failed to remove unsupported file capabilities from ${bwrap_path}" >&2
    fi
  fi
}

if ! find_c_compiler >/dev/null 2>&1; then
  if ! install_system_deps; then
    cat >&2 <<'EOF'
setup.sh: failed to install system dependencies.
This environment needs a C compiler/linker before Rust can build this repo.
EOF
    exit 1
  fi
fi

compiler="$(find_c_compiler || true)"
if [[ -z "${compiler}" ]]; then
  echo "setup.sh: no C compiler found; install gcc or clang" >&2
  exit 1
fi
export CC="${compiler}"

if ! command -v bwrap >/dev/null 2>&1; then
  if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    ${SUDO} apt-get update && ${SUDO} apt-get install -y --no-install-recommends bubblewrap libcap2-bin || true
  elif command -v dnf >/dev/null 2>&1; then
    ${SUDO} dnf install -y bubblewrap libcap || true
  elif command -v yum >/dev/null 2>&1; then
    ${SUDO} yum install -y bubblewrap libcap || true
  elif command -v apk >/dev/null 2>&1; then
    ${SUDO} apk add --no-cache bubblewrap libcap || true
  elif command -v pacman >/dev/null 2>&1; then
    ${SUDO} pacman -Sy --noconfirm bubblewrap libcap || true
  elif command -v zypper >/dev/null 2>&1; then
    ${SUDO} zypper --non-interactive install bubblewrap libcap-progs || true
  fi
fi
install_cap_tools
fix_bwrap_file_caps

if ! command -v cargo >/dev/null 2>&1; then
  if ! curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal; then
    cat >&2 <<'EOF'
setup.sh: failed to install Rust with rustup.
If this sandbox has no outbound network/DNS, use an image that already includes rustup/cargo and this repo's Rust toolchain.
EOF
    exit 1
  fi
fi

# shellcheck disable=SC1091
. "$HOME/.cargo/env" 2>/dev/null || true
if ! command -v cargo >/dev/null 2>&1; then
  echo "setup.sh: cargo is still not available after setup" >&2
  exit 1
fi

channel="$(pinned_rust_channel)"
if [[ -n "${channel}" ]] && command -v rustup >/dev/null 2>&1; then
  toolchain="$(installed_toolchain_for_channel "${channel}" || true)"
  if [[ -z "${toolchain}" ]]; then
    rustup toolchain install "${channel}" --profile minimal
    toolchain="$(installed_toolchain_for_channel "${channel}" || true)"
  fi
  if [[ -z "${toolchain}" ]]; then
    echo "setup.sh: failed to install Rust toolchain '${channel}'" >&2
    exit 1
  fi
  rustup component add rustfmt --toolchain "${toolchain}"
  export RUSTUP_TOOLCHAIN="${toolchain}"
fi

cargo fetch --locked
RUSTFLAGS="-C linker=${compiler}" cargo build --release --locked --bin build_circuit --bin eval_circuit
