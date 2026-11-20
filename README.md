<!-- SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# linux-app

The Linux app of [Vauchi](https://vauchi.app): one binary per pairing of
a display shell with Core and the Linux hardware shell
([ADR-078](https://gitlab.com/vauchi/docs)).

| Binary | Display shell | Hardware shell |
|---|---|---|
| `gvauchi` | GTK (`gtk-display`) | `linux-hw` |
| `qvauchi` | Qt (`qt-display`) | `linux-hw` |
| `vauchi-tui` | TUI | `linux-hw` |

The CLI binary follows. GTK remains the default pairing. To build and run
Qt from the workspace without GTK:

```sh
just exec-in-repo linux-app 'cargo run --no-default-features --features qt --bin qvauchi'
```

The Qt dependency pins the stable `qt-display-v0.1.3` tag. Its GUI-thread
poll advances Core invalidations and hardware observations while idle;
native events use the contract's bounded JSON decoder. Qt owns presentation
and permission dialogs, while Core owns permission state and settings
guidance. Camera preview remains separate work.

The app holds no behaviour: [`src/wiring.rs`](src/wiring.rs) starts the
hardware shell and hands it to the display shell, and CI's
`lint:no-behaviour` fails on any `Command` or `Event` match anywhere
else. Every crate in a binary pins the same contract tag, and
`deny.toml` refuses a second copy of any Vauchi crate.

```sh
cargo run --bin gvauchi
```

## Using your own relay

Every binary talks to the production relay unless told otherwise. A relay
of your own needs three things together: its address, the trust anchor
its OHTTP keys chain to (64 hex digits), and the OHTTP relay in front of
it. Without the anchor nothing syncs, and the app says so on standard
error at start.

Put them in `relay.json` in the data directory (`$XDG_DATA_HOME/vauchi`,
or `~/.local/share/vauchi`):

```json
{
  "url": "wss://relay.example.org",
  "anchor": "<64 hex digits>",
  "ohttp_relay": "https://ohttp.example.org"
}
```

`anchor` and `ohttp_relay` may be left out; any other key is refused.
The first source that exists is used whole, and sources are never mixed:

1. `relay.json` in the data directory
2. `relay_url.txt` in the data directory: an address alone
3. `VAUCHI_RELAY_URL`, with `VAUCHI_RELAY_ANCHOR` and
   `VAUCHI_OHTTP_RELAY_URL`
4. the production relay

A malformed setting stops the app at start with the reason; it does not
fall back to another relay.

This repository is GPL-3.0-or-later, like the GTK display shell. Core
and the hardware shell are AGPL-3.0-or-later, so a gvauchi binary, which
links them, is distributed under AGPL-3.0-or-later.
