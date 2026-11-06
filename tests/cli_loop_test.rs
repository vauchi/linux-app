// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use std::{collections::VecDeque, io, ops::ControlFlow, time::Duration};
use vauchi_contract::{
    AccessibilitySpec, BindingId, Command, Event, InputValue, PresentationInputKind,
    PresentationNode,
};
use vauchi_linux_app::{
    terminal::TerminalIo,
    terminal_cli::{CliTerminal, InputChunk, LineInput},
};

struct Input(VecDeque<InputChunk>);
impl LineInput for Input {
    fn read(&mut self, timeout: Duration) -> io::Result<InputChunk> {
        assert_eq!(timeout, Duration::from_millis(50));
        Ok(self.0.pop_front().expect("unexpected read"))
    }
    fn is_terminal(&self) -> bool {
        true
    }
}

fn commands(secret: bool) -> Vec<Command> {
    let fixture: serde_json::Value =
        serde_json::from_str(vauchi_contract::fixtures::presentation_contract_v1_json()).unwrap();
    let initial: Vec<Command> =
        serde_json::from_value(fixture["initial_commands"].clone()).unwrap();
    let surface = initial
        .into_iter()
        .find_map(|command| match command {
            Command::ReplaceSurface { mut surface } => {
                surface.nodes = vec![PresentationNode::Input {
                    binding_id: BindingId::new("entry").unwrap(),
                    label: "Entry".into(),
                    value: String::new(),
                    placeholder: None,
                    input_kind: if secret {
                        PresentationInputKind::Password
                    } else {
                        PresentationInputKind::Text
                    },
                    max_length: None,
                    validation_error: None,
                    enabled: true,
                    accessibility: AccessibilitySpec::label("Entry"),
                }];
                Some(surface)
            }
            _ => None,
        })
        .unwrap();
    vec![Command::ReplaceSurface { surface }]
}

fn submitted(events: ControlFlow<(), Vec<Event>>) -> String {
    let ControlFlow::Continue(events) = events else {
        panic!("input exited")
    };
    assert_eq!(events.len(), 1);
    let Event::ValueChanged {
        binding_id,
        value: InputValue::Text(value),
        ..
    } = &events[0]
    else {
        panic!("wrong event")
    };
    assert_eq!(binding_id, &BindingId::new("entry").unwrap());
    value.clone()
}

// @internal — unfinished input returns control so the bridge can deliver background observations.
#[test]
fn partial_input_yields_and_eof_submits_the_last_unterminated_line_once() {
    let mut output = Vec::new();
    let input = Input(VecDeque::from([
        InputChunk::Data(b"Alice".to_vec()),
        InputChunk::End,
    ]));
    let mut terminal = CliTerminal::new(&mut output, input, |_, _| Ok(()));
    assert_eq!(
        terminal.render(&commands(false)).unwrap(),
        ControlFlow::Continue(())
    );
    assert_eq!(
        terminal.wait_input(Duration::from_millis(50)).unwrap(),
        ControlFlow::Continue(vec![])
    );
    assert_eq!(
        submitted(terminal.wait_input(Duration::from_millis(50)).unwrap()),
        "Alice"
    );
    assert_eq!(
        terminal.wait_input(Duration::from_millis(50)).unwrap(),
        ControlFlow::Break(())
    );
}

// @internal
#[test]
fn secrets_preserve_whitespace_and_are_never_echoed() {
    let mut output = Vec::new();
    let input = Input(VecDeque::from([InputChunk::Data(b"  secret  \n".to_vec())]));
    let mut terminal = CliTerminal::new(&mut output, input, |_, _| Ok(()));
    assert_eq!(
        terminal.render(&commands(true)).unwrap(),
        ControlFlow::Continue(())
    );
    assert_eq!(
        submitted(terminal.wait_input(Duration::from_millis(50)).unwrap()),
        "  secret  "
    );
    drop(terminal);
    assert!(!String::from_utf8(output).unwrap().contains("secret"));
}

// @internal
#[test]
fn background_replacement_discards_a_partially_typed_value() {
    let mut output = Vec::new();
    let input = Input(VecDeque::from([
        InputChunk::Data(b"old".to_vec()),
        InputChunk::Data(b"fresh\n".to_vec()),
    ]));
    let mut terminal = CliTerminal::new(&mut output, input, |_, _| Ok(()));
    let mut batch = commands(false);
    assert_eq!(terminal.render(&batch).unwrap(), ControlFlow::Continue(()));
    assert_eq!(
        terminal.wait_input(Duration::from_millis(50)).unwrap(),
        ControlFlow::Continue(vec![])
    );
    let Command::ReplaceSurface { surface } = &mut batch[0] else {
        unreachable!()
    };
    surface.revision += 1;
    surface.surface_id = vauchi_contract::SurfaceId::new("replacement").unwrap();
    assert_eq!(terminal.render(&batch).unwrap(), ControlFlow::Continue(()));
    assert_eq!(
        submitted(terminal.wait_input(Duration::from_millis(50)).unwrap()),
        "fresh"
    );
}

// @internal
#[test]
fn quit_and_empty_pipe_eof_exit_without_inventing_an_event() {
    for chunk in [
        InputChunk::End,
        InputChunk::Data(vec![3]),
        InputChunk::Data(vec![4]),
    ] {
        let mut output = Vec::new();
        let mut terminal =
            CliTerminal::new(&mut output, Input(VecDeque::from([chunk])), |_, _| Ok(()));
        assert_eq!(
            terminal.render(&commands(false)).unwrap(),
            ControlFlow::Continue(())
        );
        assert_eq!(
            terminal.wait_input(Duration::from_millis(50)).unwrap(),
            ControlFlow::Break(())
        );
    }
}

// @internal
#[test]
fn invalid_utf8_and_oversized_lines_fail_explicitly() {
    for (data, kind) in [
        (vec![255, 10], io::ErrorKind::InvalidData),
        (vec![b'x'; 65537], io::ErrorKind::InvalidData),
    ] {
        let mut output = Vec::new();
        let mut terminal = CliTerminal::new(
            &mut output,
            Input(VecDeque::from([InputChunk::Data(data)])),
            |_, _| Ok(()),
        );
        assert_eq!(
            terminal.render(&commands(false)).unwrap(),
            ControlFlow::Continue(())
        );
        assert_eq!(
            terminal
                .wait_input(Duration::from_millis(50))
                .unwrap_err()
                .kind(),
            kind
        );
    }
}

// @internal — a Core event may produce no immediate command batch.
#[test]
fn buffered_input_resumes_after_an_event_without_a_repaint() {
    let mut output = Vec::new();
    let input = Input(VecDeque::from([InputChunk::Data(
        b"Alice\n1\nBob\n".to_vec(),
    )]));
    let mut terminal = CliTerminal::new(&mut output, input, |_, _| Ok(()));
    assert_eq!(
        terminal.render(&commands(false)).unwrap(),
        ControlFlow::Continue(())
    );
    assert_eq!(
        submitted(terminal.wait_input(Duration::from_millis(50)).unwrap()),
        "Alice"
    );
    assert_eq!(
        terminal.wait_input(Duration::from_millis(50)).unwrap(),
        ControlFlow::Continue(vec![])
    );
    assert_eq!(
        submitted(terminal.wait_input(Duration::from_millis(50)).unwrap()),
        "Bob"
    );
}

// @internal — identical snapshots must not erase the visible unfinished line.
#[test]
fn identical_repaint_keeps_the_pending_input_visible() {
    let mut output = Vec::new();
    let input = Input(VecDeque::from([InputChunk::Data(b"Ali".to_vec())]));
    let mut terminal = CliTerminal::new(&mut output, input, |_, _| Ok(()));
    assert_eq!(
        terminal.render(&commands(false)).unwrap(),
        ControlFlow::Continue(())
    );
    assert_eq!(
        terminal.wait_input(Duration::from_millis(50)).unwrap(),
        ControlFlow::Continue(vec![])
    );
    assert_eq!(
        terminal.render(&commands(false)).unwrap(),
        ControlFlow::Continue(())
    );
    drop(terminal);
    assert!(String::from_utf8(output).unwrap().ends_with("Entry > Ali"));
}
