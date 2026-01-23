#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/profile-callgrind.sh [options] [-- <app-args...>]

Runs GitGpui under Valgrind Callgrind with instrumentation OFF at startup,
then lets you toggle collection ON/OFF interactively.

Options:
  --bin NAME        Binary name to run (default: gitgpui-app)
  --package NAME    Cargo package to build (default: gitgpui-app)
  --release         Shortcut for --profile release
  --debug           Shortcut for --profile dev
  --profile NAME    Cargo profile to build/run (default: release)
  --features LIST   Cargo features to enable (passed to cargo build)
  --no-default-features  Disable default Cargo features
  --no-build        Do not run cargo build (fail if binary missing)
  --out FILE        Callgrind output file pattern (default: callgrind.out.%p)
                   (%p expands to PID; recommended to avoid collisions)
  --open            Open output with kcachegrind after exit (if installed)
  -h, --help        Show help

Examples:
  scripts/profile-callgrind.sh
  scripts/profile-callgrind.sh --profile dev -- --help
  scripts/profile-callgrind.sh --features ui-gpui,gix -- /path/to/repo
  scripts/profile-callgrind.sh --out callgrind.out.%p --open
EOF
}

bin_name="gitgpui-app"
package_name="gitgpui-app"
cargo_profile="release"
features=""
no_default_features=0
build=1
out_pattern="callgrind.out.%p"
open_after=0
app_args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin) bin_name="$2"; shift 2 ;;
    --package) package_name="$2"; shift 2 ;;
    --release) cargo_profile="release"; shift ;;
    --debug) cargo_profile="dev"; shift ;;
    --profile) cargo_profile="$2"; shift 2 ;;
    --features) features="$2"; shift 2 ;;
    --no-default-features) no_default_features=1; shift ;;
    --no-build) build=0; shift ;;
    --out) out_pattern="$2"; shift 2 ;;
    --open) open_after=1; shift ;;
    -h|--help) usage; exit 0 ;;
    --) shift; app_args+=("$@"); break ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"

profile_dir="$cargo_profile"
case "$cargo_profile" in
  dev) profile_dir="debug" ;;
  release) profile_dir="release" ;;
esac

bin_path="${target_dir}/${profile_dir}/${bin_name}"

if [[ $build -eq 1 && ! -x "$bin_path" ]]; then
  (
    cd "$repo_root"
    # Ensure line info for attribution in callgrind output (especially for release).
    profile_env="${cargo_profile^^}"
    profile_env="${profile_env//-/_}"
    cargo_args=(build -p "$package_name" --profile "$cargo_profile")
    [[ -n "$features" ]] && cargo_args+=(--features "$features")
    [[ $no_default_features -eq 1 ]] && cargo_args+=(--no-default-features)
    env \
      "CARGO_PROFILE_${profile_env}_DEBUG=true" \
      "CARGO_PROFILE_${profile_env}_STRIP=none" \
      cargo "${cargo_args[@]}"
  )
fi

if [[ ! -x "$bin_path" ]]; then
  echo "Binary not found or not executable: $bin_path" >&2
  echo "Build first or omit --no-build." >&2
  exit 1
fi

if ! command -v valgrind >/dev/null 2>&1; then
  echo "valgrind not found on PATH." >&2
  exit 1
fi
if ! command -v callgrind_control >/dev/null 2>&1; then
  echo "callgrind_control not found on PATH." >&2
  exit 1
fi

vgdb_dir="$(mktemp -d -t gitgpui-callgrind.XXXXXX)"
vgdb_prefix="${vgdb_dir}/vgdb-pipe"
cleanup() {
  rm -rf "$vgdb_dir" >/dev/null 2>&1 || true
}
trap cleanup EXIT

cd "$repo_root"

echo "Starting under Callgrind:"
echo "  $bin_path ${app_args[*]:-}"
echo
echo "When the app is ready, press Enter to start collecting."
echo "When you want to stop collecting, press Enter again."
echo

valgrind \
  --tool=callgrind \
  --callgrind-out-file="$out_pattern" \
  --dump-instr=yes \
  --instr-atstart=no \
  --collect-jumps=yes \
  --sigill-diagnostics=no \
  --error-limit=no \
  --vgdb=yes \
  --vgdb-error=0 \
  --vgdb-prefix="$vgdb_prefix" \
  "$bin_path" "${app_args[@]}" &
valgrind_pid=$!

out_file="${out_pattern//%p/${valgrind_pid}}"
out_path="$out_file"
[[ "$out_path" != /* ]] && out_path="${repo_root}/${out_path}"

echo "Valgrind pid: $valgrind_pid"
echo "Manual toggles (from another shell):"
echo "  callgrind_control --vgdb-prefix=\"$vgdb_prefix\" -i on  $valgrind_pid"
echo "  callgrind_control --vgdb-prefix=\"$vgdb_prefix\" -i off $valgrind_pid"
echo

read -r _ || true
enabled=0
for _try in {1..50}; do
  if callgrind_control --vgdb-prefix="$vgdb_prefix" -i on "$valgrind_pid" >/dev/null 2>&1; then
    enabled=1
    break
  fi
  sleep 0.1
done
if [[ $enabled -ne 1 ]]; then
  callgrind_control --vgdb-prefix="$vgdb_prefix" -i on "$valgrind_pid"
fi
echo "Collecting (pid $valgrind_pid). Press Enter to stop collecting."

read -r _ || true
callgrind_control --vgdb-prefix="$vgdb_prefix" -i off "$valgrind_pid" >/dev/null
echo "Instrumentation off. Quit the app to flush final output to:"
echo "  $out_path"

wait "$valgrind_pid"

if [[ $open_after -eq 1 ]]; then
  if command -v kcachegrind >/dev/null 2>&1; then
    kcachegrind "$out_path" >/dev/null 2>&1 &
  else
    echo "kcachegrind not found on PATH; output is at: $out_path" >&2
  fi
fi
