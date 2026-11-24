# Versions and releases

Windows, Linux (and later macOS) have independent version numbers, in one place:
`[workspace.metadata.versions]` in `Cargo.toml`. A release is for one system. Two systems may have the same
number by chance, so a version is always named with its system.

| | Windows | Linux |
| --- | --- | --- |
| Tag | `v0.8.0-windows` | `v0.2.0-linux` |
| Title | AnyVector 0.8.0 for Windows | AnyVector 0.2.0 for Linux |
| Files | `AnyVector-0.8.0-windows-x64-setup.exe` | `AnyVector-0.2.0-linux-x86_64.AppImage`, `AnyVector-0.2.0-linux-amd64.deb` |

## Releasing

1. Change the number of the systems that get a release, and only theirs.
2. Write `docs/RELEASE-NOTES-<version>-<system>.md` ("# AnyVector <version> for <System>"), name it in `help.rs`
   and `docs/STATUS.md`, and for Linux add the version to the metainfo.xml.
3. `cargo run -p xtask -- versions` updates the list in the README and writes the text of each release (the
   list, then its notes) in `target/release-text`.
4. Merge, tag, and publish the release with that text. The CI of a tag builds its own system only. The Linux
   files come from the artifact `AnyVector-linux-packages` of the tag's run; the Windows setup is built locally.
5. Mark the newest release, whatever its system, as GitHub's latest: its text starts with the list of every
   system's latest version.

## Who is told of it

- AnyVector looks at the releases of its own system only (the end of the tag), so a release for Linux says
  nothing on Windows, and the other way round. The tags up to `v0.6.2`, without a system, are Windows'.
- 0.7.0 for Windows compares the releases that have a setup. 0.6.2 and older read GitHub's latest release: while
  that is another system's, they find nothing new, and they find the next Windows release while it is the latest.
- The system comes at the end of the tag and the setup's name ends in `-setup.exe`: because
  with `windows-v0.8.0` the versions up to 0.7.0 would never see an update again.
- Help ▸ Updates shows a release's notes without the list of every system's versions.
- A report (Help ▸ Report a Bug, Submit Ideas and Feedback) always names its system, and the issue gets the label
  `windows`, `linux` or `macos`.

## When the systems are unified

Give them the same number, such as 1.0.0, in the table of `Cargo.toml`. A system's number can only grow: AnyVector
never offers a lower one.

## A new system

Add it to `[workspace.metadata.versions]`, to `SYSTEMS` in `updater.rs` (with `is_for` and `asset_for`), to
`system()` in `xtask/src/versions.rs` and to `help.rs`. macOS is in `SYSTEMS` and in the reports already.
