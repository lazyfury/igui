#!/usr/bin/env bash
#
# rename-crate-prefix.sh — deterministic internal crate rename for the quill
# workspace.
#
# This is the codified form of the `draw_*` -> `cobbled_*` rename (which also
# moved `quill_app` / `quill_winit` / `quill_headless` to `cobbled_*`). The
# public `quill` facade is never touched.
#
# It performs the *deterministic* half:
#   1. rewrites every full crate name, word-bounded, in tracked text files
#      (never the bare `draw_` prefix, so `draw_line` / `draw_rect` /
#      `draw_list` are untouched);
#   2. `git mv`s the crate directories (directory name == crate name);
# then prints the manual follow-ups that a `cargo check` / `test` round will
# otherwise surface.
#
# Usage:
#   scripts/rename-crate-prefix.sh                 # dry run (default)
#   scripts/rename-crate-prefix.sh --apply         # apply
#   scripts/rename-crate-prefix.sh --invert        # swap old/new (e.g. revert)
#   scripts/rename-crate-prefix.sh --apply --invert
#
# Run it from anywhere inside the repository. It never commits.
# Written for bash 3.2 (macOS default): no associative arrays.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# old -> new. Forward = the cobbled rename. `--invert` swaps each pair, so the
# same table reverts `cobbled_*` back to `draw_*` / `quill_app` / ... .
mapping=(
  "draw_core cobbled_core"
  "draw_render cobbled_render"
  "draw_scene cobbled_scene"
  "draw_ui cobbled_ui"
  "draw_theme cobbled_theme"
  "draw_components cobbled_components"
  "draw_font cobbled_font"
  "draw_svg cobbled_svg"
  "draw_assets cobbled_assets"
  "draw_anim cobbled_anim"
  "draw_game cobbled_game"
  "draw_backend_canvas cobbled_backend_canvas"
  "draw_backend_recording cobbled_backend_recording"
  "draw_backend_wgpu cobbled_backend_wgpu"
  "draw_wasm cobbled_wasm"
  "draw_ffi cobbled_ffi"
  "draw_profile cobbled_profile"
  "draw_debug_ui cobbled_debug_ui"
  "draw_bench_suite cobbled_bench_suite"
  "draw_bench cobbled_bench"
  "quill_app cobbled_app"
  "quill_headless cobbled_headless"
  "quill_winit cobbled_winit"
)

usage() {
  sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
}

mode=dry
invert=0
for arg in "$@"; do
  case "$arg" in
    --apply) mode=apply ;;
    --dry-run) mode=dry ;;
    --invert) invert=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; usage; exit 2 ;;
  esac
done

olds=()
news=()
for pair in "${mapping[@]}"; do
  a=${pair%% *}
  b=${pair##* }
  if ((invert)); then
    olds+=("$b"); news+=("$a")
  else
    olds+=("$a"); news+=("$b")
  fi
done

echo "rename-crate-prefix: mode=$mode invert=$invert"
i=0
while ((i < ${#olds[@]})); do
  printf '  %-28s -> %s\n' "${olds[$i]}" "${news[$i]}"
  i=$((i + 1))
done

if [[ "$mode" == dry ]]; then
  echo
  echo "dry run; re-run with --apply to modify the tree."
  exit 0
fi

# 1) textual rewrite of full crate names (word-bounded).
perl_expr=""
i=0
while ((i < ${#olds[@]})); do
  perl_expr+="s/\\b${olds[$i]}\\b/${news[$i]}/g;"
  i=$((i + 1))
done

pattern=$(printf '%s\n' "${olds[@]}" | paste -sd'|' -)
files=$(rg -l \
  -g '!target/**' -g '!examples/*/build/**' -g '!Cargo.lock' \
  -g '*.rs' -g '*.toml' -g '*.md' -g '*.h' -g '*.hpp' -g '*.cpp' -g '*.txt' -g '*.sh' \
  "$pattern" . || true)

if [[ -n "$files" ]]; then
  # shellcheck disable=SC2086
  perl -0pi -e "$perl_expr" $files
fi

# 2) directory moves (the directory basename equals the crate name).
i=0
while ((i < ${#olds[@]})); do
  dir=$(find crates -type d -name "${olds[$i]}" | head -n 1 || true)
  if [[ -n "$dir" ]]; then
    git mv "$dir" "$(dirname "$dir")/${news[$i]}"
  fi
  i=$((i + 1))
done

cat <<'MANUAL'
rename-crate-prefix: done. Manual follow-ups (find what the compiler misses):
  - `cargo fmt --all` (import order changes with the prefix), then the gate.
  - CMake / C ABI hosts: static-lib file name (`lib<name>.a`), custom target
    names, `-p` flags in build scripts/CMakeLists.
  - Docs: `draw_*` / old-prefix globs that a word-bounded replace does not match.
  - `pub const CRATE` values and their identity tests, log-prefix strings.
MANUAL
