<!-- SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# linux-app

The Linux app of [Vauchi](https://vauchi.app): one binary per pairing of
a display shell with Core and the Linux hardware shell
([ADR-078](https://gitlab.com/vauchi/docs)).

| Binary | Display shell | Hardware shell |
|---|---|---|
| `gvauchi` | GTK (`linux-gtk`, renamed `gtk-display` in step 4) | `linux-hw` |

Qt, CLI and TUI follow (`qvauchi`, `vauchi`, `vauchi-tui`).

The app holds no behaviour: [`src/wiring.rs`](src/wiring.rs) starts the
hardware shell and hands it to the display shell, and CI's
`lint:no-behaviour` fails on any `Command` or `Event` match anywhere
else. Every crate in a binary pins the same contract tag, and
`deny.toml` refuses a second copy of any Vauchi crate.

```sh
cargo run --bin gvauchi
```

This repository is GPL-3.0-or-later, like the GTK display shell. Core
and the hardware shell are AGPL-3.0-or-later, so a gvauchi binary, which
links them, is distributed under AGPL-3.0-or-later.
