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

To undo, revert the commit (`git revert c512a8e`) or run the script with the
previous prefix, e.g. `scripts/rename-crate-prefix.sh cobbled --apply`.

The same transformation is scripted (parameterized by the new prefix) in
`scripts/rename-crate-prefix.sh`:

```bash
scripts/rename-crate-prefix.sh <new-prefix>          # dry run
scripts/rename-crate-prefix.sh <new-prefix> --apply  # rename
```
