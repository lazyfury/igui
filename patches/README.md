# patches

Portable snapshots of larger refactors, kept so a checkout without the commit
history can still review or replay them.

## `cobbled-prefix.patch`

The internal crate-prefix rename `draw_*` / `quill_app|winit|headless` ->
`cobbled_*` (the public `quill` facade is unchanged). It is the diff of commit
`c512a8e`.

```bash
# inspect
git apply --stat  patches/cobbled-prefix.patch
git apply --check patches/cobbled-prefix.patch   # against the pre-rename tree

# apply (working tree; then cargo fmt --all + the workspace gate)
git apply patches/cobbled-prefix.patch

# or replay with history
git am patches/cobbled-prefix.patch
```

To undo, revert the commit (`git revert c512a8e`) or run the script with
`--invert` (see `scripts/rename-crate-prefix.sh`).

The same transformation is scripted in `scripts/rename-crate-prefix.sh`
(dry-run by default; `--apply`, `--invert`).
