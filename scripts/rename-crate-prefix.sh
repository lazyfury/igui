#!/usr/bin/env bash
#
# rename-crate-prefix.sh — rename the internal crate prefix across the quill
# workspace.
#
# Usage:
#   scripts/rename-crate-prefix.sh <new-prefix> [--apply] [--old p1,p2,...]
#
# Examples:
#   scripts/rename-crate-prefix.sh xxkit                 # dry run (default)
#   scripts/rename-crate-prefix.sh xxkit --apply         # cobbled_* -> xxkit_*
#   scripts/rename-crate-prefix.sh cobbled --apply       # revert xxkit_* -> cobbled_*
#
# The public `quill` facade (package `quill`, directory `quill/`) is never
# renamed. Internal crates are discovered from `crates/**/Cargo.toml`, and the
# old prefix is auto-detected from their names (or passed with `--old`,
# comma-separated, e.g. `--old draw,quill`).
#
# It performs the deterministic half:
#   1. rewrites every full crate name in tracked text files. A *bare* prefix
#      (`draw_`) is never replaced, so `draw_line` / `draw_rect` / `draw_list`
#      are safe. Full names are replaced as substrings (longest first), so
#      `lib<name>.a` and `<name>_build` follow too;
#   2. `git mv`s the crate directories (directory name == crate name);
# then prints the manual follow-ups that a `cargo check` / `test` round will
# otherwise surface.
#
# Run from anywhere inside the repository. It never commits. bash 3.2 compatible.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

usage() {
  sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'
}

new=""
old=""
mode=dry
while (($#)); do
  case "$1" in
    --apply) mode=apply ;;
    --dry-run) mode=dry ;;
    --old) shift; old=${1:-} ;;
    --old=*) old=${1#--old=} ;;
    -h|--help) usage; exit 0 ;;
    -*) echo "unknown option: $1" >&2; usage; exit 2 ;;
    *) if [[ -z "$new" ]]; then new=$1; else echo "unexpected argument: $1" >&2; exit 2; fi ;;
  esac
  shift
done

if [[ -z "$new" ]]; then
  usage; exit 2
fi
if [[ ! "$new" =~ ^[a-z][a-z0-9_]*$ ]]; then
  echo "new prefix must match ^[a-z][a-z0-9_]*\$ (got '$new')" >&2
  exit 2
fi

# 1) discover internal crates (everything under crates/, never the facade).
olds=()
suffixes=()
first_segment=""
mixed=0
while IFS= read -r manifest; do
  name=$(grep -m1 '^name = ' "$manifest" | sed -E 's/^name = "(.*)"/\1/')
  [[ -z "$name" ]] && continue
  segment=${name%%_*}
  if [[ -z "$first_segment" ]]; then
    first_segment=$segment
  elif [[ "$segment" != "$first_segment" ]]; then
    mixed=1
  fi
  olds+=("$name")
done < <(find crates -name Cargo.toml | sort)

if (( ${#olds[@]} == 0 )); then
  echo "no crates found under crates/" >&2
  exit 1
fi

# 2) decide the source prefixes to strip.
old_prefixes=()
if [[ -n "$old" ]]; then
  IFS=',' read -r -a old_prefixes <<< "$old"
elif (( mixed == 0 )); then
  old_prefixes=("$first_segment")
else
  echo "internal crates mix prefixes; pass --old <p1,p2,...>" >&2
  exit 2
fi

# 3) build old -> new, preserving the suffix after the stripped prefix.
news=()
renamed=()
for name in "${olds[@]}"; do
  suffix=""
  for p in "${old_prefixes[@]}"; do
    if [[ "$name" == "${p}_"* ]]; then
      suffix=${name#"${p}_"}
      break
    fi
  done
  if [[ -z "$suffix" ]]; then
    continue
  fi
  renamed+=("$name")
  news+=("${new}_${suffix}")
done

if (( ${#renamed[@]} == 0 )); then
  echo "no crates match the source prefix(es) '${old_prefixes[*]}'" >&2
  exit 1
fi

echo "rename-crate-prefix: ${old_prefixes[*]}_* -> ${new}_* (mode=$mode, ${#renamed[@]} crates)"
i=0
while ((i < ${#renamed[@]})); do
  printf '  %-28s -> %s\n' "${renamed[$i]}" "${news[$i]}"
  i=$((i + 1))
done

if [[ "$mode" == dry ]]; then
  echo
  echo "dry run; re-run with --apply to modify the tree."
  exit 0
fi

# 4) rewrite full crate names as substrings, longest first (so `x_bench` does
#    not clobber `x_bench_suite`, and `libx_ffi.a` / `x_ffi_build` follow).
order=$(for i in "${!renamed[@]}"; do echo "$i ${#renamed[$i]}"; done | sort -k2,2nr | awk '{print $1}')
perl_expr=""
for i in $order; do
  from=${renamed[$i]}
  to=${news[$i]}
  perl_expr+="s/\Q${from}\E/${to}/g;"
done

files=$(rg -l \
  -g '!target/**' -g '!examples/*/build/**' -g '!Cargo.lock' \
  -g '!scripts/**' -g '!patches/**' \
  -g '*.rs' -g '*.toml' -g '*.md' -g '*.h' -g '*.hpp' -g '*.cpp' -g '*.txt' -g '*.sh' \
  "$(printf '%s\n' "${renamed[@]}" | paste -sd'|' -)" . || true)

if [[ -n "$files" ]]; then
  # shellcheck disable=SC2086
  perl -0pi -e "$perl_expr" $files
fi

# 5) directory moves (the directory basename equals the crate name).
i=0
while ((i < ${#renamed[@]})); do
  dir=$(find crates -type d -name "${renamed[$i]}" | head -n 1 || true)
  if [[ -n "$dir" ]]; then
    git mv "$dir" "$(dirname "$dir")/${news[$i]}"
  fi
  i=$((i + 1))
done

cat <<'MANUAL'
rename-crate-prefix: done. Manual follow-ups (find what the compiler misses):
  - `cargo fmt --all` (import order changes with the prefix), then the gate:
    cargo check --workspace && cargo test --workspace && cargo bench --workspace --no-run
  - `pub const CRATE` values and their identity tests, log-prefix strings.
  - Prose / wildcards the text pass did not match, and any generated artifact.
MANUAL
