# gedoens

gedoens is a small secrets manager for teams that use [SOPS](https://getsops.io).
Git does the syncing and the history, your keys decide who can read what, and
gedoens is the interface on top: read and change values, grant and revoke access,
merge conflicts after a pull.

There is no server and no account. Everything runs on your machine.

Part of the same family of small tools as kuebel.

## What it does

- **Files**: finds every SOPS file in a folder (YAML, JSON, ENV, INI). A value is
  decrypted only when you open its row. Enter saves, and every change can be undone.
- **Access**: one view answers "who may read what". Tap an area to grant access.
  Revoking updates the files and rotates their data keys in one step.
- **Names**: every key gets a name, stored as a comment next to the key in
  `.sops.yaml` (`- age1…  # Anna`).
- **Drift**: a file whose recipients differ from its rule shows up as an open item
  with one button to fix it.
- **Merging**: after a pull, two edits to the same file always collide in the
  `sops` block. gedoens compares the three versions value by value in plaintext,
  merges what does not overlap and asks only where both sides changed the same value.

## Compatibility

Whoever does not use gedoens notices nothing.

- Every file stays a plain SOPS file. gedoens only writes through the official
  `sops` binary and adds no fields of its own.
- gedoens keeps no file of its own in your repository.
- Your key lives where SOPS looks for it (`keys.txt`), so it works in the app and
  in the terminal alike.
- Rules that use key groups or YAML aliases are shown but never rewritten.

## Requirements

- `sops` 3.10 or newer and `git` on your machine. gedoens looks next to the app,
  in `PATH` and in the usual places (Homebrew, `/usr/local/bin`, Nix). Override
  with `GEDOENS_SOPS` and `GEDOENS_GIT`.
- An age key. If you have none, gedoens creates one on first start.

## Architecture

The code follows domain-driven design. Dependencies point inwards only.

```
crates/domain          Pure rules: recipients, areas, drift, three-way merge.
                       No files, no processes. Tested without any tooling.
crates/application     Use cases and the ports they need. A value leaves this
                       layer only one at a time and only on request.
crates/infrastructure  Adapters: sops, git, file system, .sops.yaml, keys.txt.
src-tauri              The shell: one window and thin commands.
ui                     Plain HTML, CSS and JavaScript. No build step.
```

Three decisions worth knowing:

- gedoens does not reimplement SOPS. It drives the official binary and passes
  secret values through stdin, never through the command line.
- The backend holds no plaintext between requests. Copying a value happens in the
  backend, and the clipboard is cleared after 30 seconds.
- `.sops.yaml` is read with a YAML parser but edited as text: only the `age` block
  of the affected rule is replaced, everything else stays byte for byte.

## Development

```sh
cargo test                 # domain, application and adapters
cargo install tauri-cli --version "^2" --locked
cargo tauri dev            # run the app
cargo tauri build          # build installers
```

`gedoens path/to/repo` opens a folder right away.

`cargo test` includes one end-to-end test against the real `sops` and `git`.
It is skipped when either is missing.

## Status

Early. Working: files, values, access, names, drift, merging.
Not built yet: bundled `sops`, rotation reminders, the checklist of values to
replace after removing someone, a git merge driver, app lock.

Tested on Linux only so far. The bundle identifier in
`src-tauri/tauri.conf.json` is a placeholder to review before the first release.

## License

Not chosen yet. The bundled fonts Outfit and JetBrains Mono are licensed under
the SIL Open Font License, see `ui/fonts`.
