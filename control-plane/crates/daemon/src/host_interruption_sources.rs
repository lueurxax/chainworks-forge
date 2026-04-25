use anyhow::Result;
use chrono::{DateTime, Utc};
use engine::host_interruption::{
    HostInterruptionEvent, HostInterruptionKind, HostInterruptionRecoverySummary,
    HostInterruptionService,
};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;
use tracing::{info, warn};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeHostInterruptionEvent {
    SystemSleepWake {
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
    },
    NetworkMigration {
        observed_at: DateTime<Utc>,
    },
}

pub fn spawn_native_host_interruption_monitor(service: HostInterruptionService) -> JoinHandle<()> {
    let (tx, mut rx) = mpsc::channel(64);
    start_platform_sources(tx);

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Err(error) = record_native_host_interruption_event(&service, event).await {
                warn!(
                    error = %error,
                    "native host interruption recovery failed"
                );
            }
        }
    })
}

async fn record_native_host_interruption_event(
    service: &HostInterruptionService,
    event: NativeHostInterruptionEvent,
) -> Result<HostInterruptionRecoverySummary> {
    service
        .record_and_requeue(host_interruption_event_for_native(event))
        .await
}

fn host_interruption_event_for_native(event: NativeHostInterruptionEvent) -> HostInterruptionEvent {
    match event {
        NativeHostInterruptionEvent::SystemSleepWake {
            started_at,
            ended_at,
        } => HostInterruptionEvent {
            kind: HostInterruptionKind::SystemSleep,
            started_at,
            ended_at: Some(ended_at),
            monotonic_gap_ms: None,
            wall_clock_gap_ms: Some((ended_at - started_at).num_milliseconds().max(0)),
            details_json: Some(r#"{"source":"system_sleep_wake"}"#.into()),
        },
        NativeHostInterruptionEvent::NetworkMigration { observed_at } => HostInterruptionEvent {
            kind: HostInterruptionKind::NetworkMigration,
            started_at: observed_at,
            ended_at: Some(observed_at),
            monotonic_gap_ms: None,
            wall_clock_gap_ms: None,
            details_json: Some(r#"{"source":"network_path_change"}"#.into()),
        },
    }
}

#[cfg(target_os = "macos")]
fn start_platform_sources(tx: mpsc::Sender<NativeHostInterruptionEvent>) {
    macos::start_system_sleep_wake_source(tx.clone());
    macos::start_network_migration_source(tx);
}

#[cfg(not(target_os = "macos"))]
fn start_platform_sources(_tx: mpsc::Sender<NativeHostInterruptionEvent>) {
    info!("native host interruption sources unavailable on this platform");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::ptr;
    use std::thread;
    use std::time::{Duration as StdDuration, Instant};

    use super::*;

    type CFRunLoopRef = *mut c_void;
    type CFRunLoopSourceRef = *mut c_void;
    type CFStringRef = *const c_void;
    type IONotificationPortRef = *mut c_void;
    type IoObject = u32;
    type IoConnect = u32;
    type Natural = u32;
    type KernReturn = i32;

    const K_IO_MESSAGE_CAN_SYSTEM_SLEEP: Natural = 0x0002_7000;
    const K_IO_MESSAGE_SYSTEM_WILL_SLEEP: Natural = 0x0002_8000;
    const K_IO_MESSAGE_SYSTEM_HAS_POWERED_ON: Natural = 0x0003_0000;
    const RTM_ADD: u8 = 0x1;
    const RTM_DELETE: u8 = 0x2;
    const RTM_CHANGE: u8 = 0x3;
    const RTM_NEWADDR: u8 = 0xc;
    const RTM_DELADDR: u8 = 0xd;
    const RTM_IFINFO: u8 = 0xe;
    const RTM_IFANNOUNCE: u8 = 0xf;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFRunLoopDefaultMode: CFStringRef;
        fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        fn CFRunLoopRun();
        fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    }

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IORegisterForSystemPower(
            refcon: *mut c_void,
            the_port_ref: *mut IONotificationPortRef,
            callback: extern "C" fn(*mut c_void, IoObject, Natural, *mut c_void),
            notifier: *mut IoObject,
        ) -> IoConnect;
        fn IONotificationPortGetRunLoopSource(port: IONotificationPortRef) -> CFRunLoopSourceRef;
        fn IOAllowPowerChange(kernel_port: IoConnect, notification_id: isize) -> KernReturn;
    }

    pub(super) fn start_system_sleep_wake_source(tx: mpsc::Sender<NativeHostInterruptionEvent>) {
        if let Err(error) = thread::Builder::new()
            .name("cw-host-sleep-wake-source".into())
            .spawn(move || {
                let state = Box::new(PowerNotificationState {
                    tx,
                    root_port: 0,
                    last_sleep_started_at: None,
                });
                // SAFETY: the callback state is owned by the CFRunLoop thread for
                // its lifetime. It is reclaimed only on registration failure.
                let state_ptr = Box::into_raw(state);
                let mut notify_port: IONotificationPortRef = ptr::null_mut();
                let mut notifier: IoObject = 0;
                let root_port = unsafe {
                    IORegisterForSystemPower(
                        state_ptr.cast(),
                        &mut notify_port,
                        power_notification_callback,
                        &mut notifier,
                    )
                };

                if root_port == 0 || notify_port.is_null() {
                    warn!("failed to register macOS system sleep/wake notification source");
                    unsafe {
                        drop(Box::from_raw(state_ptr));
                    }
                    return;
                }

                unsafe {
                    (*state_ptr).root_port = root_port;
                    let source = IONotificationPortGetRunLoopSource(notify_port);
                    CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
                    info!("native host interruption sleep/wake source registered");
                    CFRunLoopRun();
                }
            })
        {
            warn!(
                error = %error,
                "failed to spawn macOS system sleep/wake notification thread"
            );
        }
    }

    pub(super) fn start_network_migration_source(tx: mpsc::Sender<NativeHostInterruptionEvent>) {
        if let Err(error) = thread::Builder::new()
            .name("cw-host-network-source".into())
            .spawn(move || {
                let fd = unsafe { libc::socket(libc::PF_ROUTE, libc::SOCK_RAW, libc::AF_UNSPEC) };
                if fd < 0 {
                    warn!("failed to open macOS route socket for network migration source");
                    return;
                }
                info!("native host interruption network migration source registered");

                let mut buf = [0u8; 4096];
                let mut last_sent_at: Option<Instant> = None;
                loop {
                    let read =
                        unsafe { libc::read(fd, buf.as_mut_ptr().cast::<c_void>(), buf.len()) };
                    if read < 0 {
                        warn!("macOS route socket read failed; network migration source stopped");
                        unsafe {
                            libc::close(fd);
                        }
                        return;
                    }
                    if read < 4 {
                        continue;
                    }

                    let message_type = buf[3];
                    if is_network_route_message(message_type) {
                        let now = Instant::now();
                        if last_sent_at.is_some_and(|sent_at| {
                            now.duration_since(sent_at) < StdDuration::from_secs(2)
                        }) {
                            continue;
                        }
                        last_sent_at = Some(now);
                        let event = NativeHostInterruptionEvent::NetworkMigration {
                            observed_at: Utc::now(),
                        };
                        match tx.try_send(event) {
                            Ok(()) => {}
                            Err(TrySendError::Full(_)) => {
                                warn!("dropping coalesced network migration event; host interruption channel full");
                            }
                            Err(TrySendError::Closed(_)) => {
                                unsafe {
                                    libc::close(fd);
                                }
                                return;
                            }
                        }
                    }
                }
            })
        {
            warn!(
                error = %error,
                "failed to spawn macOS network migration notification thread"
            );
        }
    }

    struct PowerNotificationState {
        tx: mpsc::Sender<NativeHostInterruptionEvent>,
        root_port: IoConnect,
        last_sleep_started_at: Option<DateTime<Utc>>,
    }

    extern "C" fn power_notification_callback(
        refcon: *mut c_void,
        _service: IoObject,
        message_type: Natural,
        message_argument: *mut c_void,
    ) {
        let state = unsafe { &mut *(refcon as *mut PowerNotificationState) };
        match message_type {
            K_IO_MESSAGE_CAN_SYSTEM_SLEEP | K_IO_MESSAGE_SYSTEM_WILL_SLEEP => {
                if message_type == K_IO_MESSAGE_SYSTEM_WILL_SLEEP {
                    state.last_sleep_started_at = Some(Utc::now());
                }
                unsafe {
                    IOAllowPowerChange(state.root_port, message_argument as isize);
                }
            }
            K_IO_MESSAGE_SYSTEM_HAS_POWERED_ON => {
                let ended_at = Utc::now();
                let started_at = state.last_sleep_started_at.take().unwrap_or(ended_at);
                let _ = state
                    .tx
                    .try_send(NativeHostInterruptionEvent::SystemSleepWake {
                        started_at,
                        ended_at,
                    });
            }
            _ => {}
        }
    }

    fn is_network_route_message(message_type: u8) -> bool {
        matches!(
            message_type,
            RTM_ADD
                | RTM_DELETE
                | RTM_CHANGE
                | RTM_NEWADDR
                | RTM_DELADDR
                | RTM_IFINFO
                | RTM_IFANNOUNCE
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_event_bridge_maps_network_migration_event() {
        let observed_at = Utc::now();
        let event =
            host_interruption_event_for_native(NativeHostInterruptionEvent::NetworkMigration {
                observed_at,
            });

        assert_eq!(event.kind, HostInterruptionKind::NetworkMigration);
        assert_eq!(event.started_at, observed_at);
        assert_eq!(event.ended_at, Some(observed_at));
        assert_eq!(event.monotonic_gap_ms, None);
        assert_eq!(event.wall_clock_gap_ms, None);
        assert_eq!(
            event.details_json.as_deref(),
            Some(r#"{"source":"network_path_change"}"#)
        );
    }

    #[test]
    fn native_event_bridge_maps_system_sleep_wake_event() {
        let ended_at = Utc::now();
        let event =
            host_interruption_event_for_native(NativeHostInterruptionEvent::SystemSleepWake {
                started_at: ended_at - chrono::Duration::seconds(30),
                ended_at,
            });

        assert_eq!(event.kind, HostInterruptionKind::SystemSleep);
        assert_eq!(event.ended_at, Some(ended_at));
        assert_eq!(event.monotonic_gap_ms, None);
        assert_eq!(event.wall_clock_gap_ms, Some(30_000));
        assert_eq!(
            event.details_json.as_deref(),
            Some(r#"{"source":"system_sleep_wake"}"#)
        );
    }
}
