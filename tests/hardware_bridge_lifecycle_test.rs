// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::RefCell,
    future::Future,
    net::TcpListener,
    pin::{Pin, pin},
    rc::Rc,
    sync::{Arc, mpsc},
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};
use vauchi_contract::{
    BleAdvertisementObservation, BleServiceUuid, Command, DirectEndpoint, Event, direct,
    executor::DeliveryError,
};
use vauchi_linux_app::{engine::CommandEngine, wiring::HardwareBridge};
use vauchi_linux_hw::{
    Backends, Discovery, EventSender, HardwareFailure, HostDevices, LinuxHardware, TcpDirect,
};

const DEADLINE: Duration = Duration::from_secs(5);

struct ScriptedCore {
    initial: Vec<Command>,
    on_wakeup: Vec<Command>,
    events: Rc<RefCell<Vec<Event>>>,
}

impl CommandEngine for ScriptedCore {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        let response = if matches!(event, Event::WakeupFired) {
            self.on_wakeup.clone()
        } else {
            vec![]
        };
        self.events.borrow_mut().push(event);
        response
    }

    fn initial_commands(&self) -> Vec<Command> {
        self.initial.clone()
    }
}

struct Released(mpsc::Sender<()>);

impl Drop for Released {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

struct PendingScan {
    ready: mpsc::Sender<(String, EventSender)>,
    released: mpsc::Sender<()>,
}

fn peer() -> Event {
    Event::BleAdvertisementObserved(
        BleAdvertisementObservation::new(
            "peer".into(),
            -40,
            vec![BleServiceUuid::Uuid16([0xab, 0xcd])],
        )
        .unwrap(),
    )
}

impl Discovery for PendingScan {
    fn scan(
        &self,
        service: String,
        mut events: EventSender,
    ) -> Pin<Box<dyn Future<Output = Result<(), HardwareFailure>> + Send + '_>> {
        Box::pin(async move {
            let _released = Released(self.released.clone());
            events.send(peer()).await.unwrap();
            self.ready.send((service, events)).unwrap();
            std::future::pending().await
        })
    }
}

struct ScanFixture {
    bridge: HardwareBridge<ScriptedCore>,
    callback: EventSender,
    released: mpsc::Receiver<()>,
    observed: Rc<RefCell<Vec<Event>>>,
}

fn scanning() -> ScanFixture {
    let (ready, started) = mpsc::channel();
    let (released, stopped) = mpsc::channel();
    let hardware = LinuxHardware::with_backends(
        Backends {
            discovery: Some(Arc::new(PendingScan { ready, released })),
            ..Backends::default()
        },
        HostDevices {
            bluetooth_adapter: true,
            ..HostDevices::default()
        },
    )
    .unwrap();
    let core = ScriptedCore {
        initial: vec![Command::BleStartScanning {
            service_uuid: "prepared-service".into(),
        }],
        on_wakeup: vec![Command::BleStopScanning],
        events: Rc::default(),
    };
    let observed = core.events.clone();
    let mut bridge = HardwareBridge::new(core, hardware).unwrap();
    assert!(bridge.wait_for_event(DEADLINE).unwrap());
    let (service, callback) = started.recv_timeout(DEADLINE).unwrap();
    assert_eq!(service, "prepared-service");
    assert_eq!(observed.borrow().last(), Some(&peer()));
    ScanFixture {
        bridge,
        callback,
        released: stopped,
        observed,
    }
}

fn late_delivery(sender: &mut EventSender) -> Poll<Result<(), DeliveryError>> {
    let mut future = pin!(sender.send(peer()));
    future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
}

// @internal — #1029: Core's stop crosses the app boundary and rejects late callbacks.
#[test]
fn stop_releases_the_scan_and_rejects_late_observations() {
    let mut f = scanning();
    assert_eq!(f.released.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert_eq!(late_delivery(&mut f.callback), Poll::Ready(Ok(())));
    f.bridge.dispatch(Event::WakeupFired).unwrap();
    assert_eq!(f.released.recv_timeout(DEADLINE), Ok(()));
    assert_eq!(
        late_delivery(&mut f.callback),
        Poll::Ready(Err(DeliveryError::Inactive))
    );
    assert!(!f.bridge.poll().unwrap());
    assert_eq!(f.observed.borrow().len(), 3);
}

// @internal — #1029: dropping the app bridge releases its pending native future.
#[test]
fn dropping_the_bridge_releases_the_scan_and_closes_its_callback() {
    let mut f = scanning();
    assert_eq!(f.released.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert_eq!(late_delivery(&mut f.callback), Poll::Ready(Ok(())));
    drop(f.bridge);
    assert_eq!(f.released.recv_timeout(DEADLINE), Ok(()));
    assert_eq!(
        late_delivery(&mut f.callback),
        Poll::Ready(Err(DeliveryError::Closed))
    );
    assert_eq!(f.observed.borrow().len(), 2);
}

// @internal — #1029: the pinned native TCP backend works while input stays pending.
#[test]
fn native_loopback_exchange_reaches_core_before_any_input_arrives() {
    for initiator in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = DirectEndpoint {
            port: listener.local_addr().unwrap().port(),
            fallback_host: "127.0.0.1".into(),
            browse_timeout_ms: 0,
            io_timeout_ms: 2_000,
            ..DirectEndpoint::default()
        };
        let peer = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            direct::exchange(&mut stream, b"peer card", !initiator).unwrap()
        });
        let hardware = LinuxHardware::with_backends(
            Backends {
                direct: Some(Arc::new(TcpDirect)),
                ..Backends::default()
            },
            HostDevices::default(),
        )
        .unwrap();
        let core = ScriptedCore {
            initial: vec![Command::DirectSend {
                payload: b"app card".to_vec(),
                is_initiator: initiator,
                endpoint,
            }],
            on_wakeup: vec![],
            events: Rc::default(),
        };
        let observed = core.events.clone();
        let (_pending_input, input) = mpsc::channel::<String>();
        let mut bridge = HardwareBridge::new(core, hardware).unwrap();
        assert_eq!(input.try_recv(), Err(mpsc::TryRecvError::Empty));
        assert!(bridge.wait_for_event(DEADLINE).unwrap());
        assert_eq!(
            observed.borrow().last(),
            Some(&Event::DirectPayloadReceived {
                data: b"peer card".to_vec()
            })
        );
        assert_eq!(input.try_recv(), Err(mpsc::TryRecvError::Empty));
        assert_eq!(peer.join().unwrap(), b"app card");
        assert_eq!(bridge.take_commands(), vec![]);
    }
}
