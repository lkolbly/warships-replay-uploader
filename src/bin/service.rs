#[macro_use]
extern crate windows_service;

use std::ffi::OsString;
use std::sync::mpsc;
use std::time::Duration;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;

use warships_replay_poller::upload_replays;

define_windows_service!(ffi_service_main, my_service_main);

fn my_service_main(arguments: Vec<OsString>) {
    if let Err(_e) = run_service(arguments) {
        // Handle errors in some way.
    }
}

fn run_service(_arguments: Vec<OsString>) -> Result<(), windows_service::Error> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop => {
                shutdown_tx.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register system service event handler
    let status_handle =
        service_control_handler::register("warships_replay_uploader", event_handler)?;

    let next_status = ServiceStatus {
        process_id: None,
        // Should match the one from system service registry
        service_type: ServiceType::OWN_PROCESS,
        // The new state
        current_state: ServiceState::Running,
        // Accept stop events when running
        controls_accepted: ServiceControlAccept::STOP,
        // Used to report an error when starting or stopping only, otherwise must be zero
        exit_code: ServiceExitCode::Win32(0),
        // Only used for pending states, otherwise must be zero
        checkpoint: 0,
        // Only used for pending states, otherwise must be zero
        wait_hint: Duration::default(),
    };

    // Tell the system that the service is running now
    status_handle.set_service_status(next_status)?;

    let mut state = std::collections::HashSet::new();
    loop {
        let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
        let cur_ver =
            match hklm.open_subkey("Software\\PillowComputing\\WorldOfWarshipsReplayUploader") {
                Ok(x) => x,
                _ => {
                    break;
                }
            };
        let replay_path: String = match cur_ver.get_value("ReplayPath") {
            Ok(x) => x,
            _ => {
                break;
            }
        };
        let base_url: String = match cur_ver.get_value("UploadServerBase") {
            Ok(x) => x,
            _ => {
                break;
            }
        };
        state = upload_replays(&replay_path, &base_url, state)
            .unwrap_or(std::collections::HashSet::new());

        // Poll shutdown event. Wakeup every 30 minutes to poll the replay directory.
        match shutdown_rx.recv_timeout(Duration::from_secs(60 * 30)) {
            // Break the loop either upon stop or channel disconnect
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,

            // Continue work if no events were received within the timeout
            Err(mpsc::RecvTimeoutError::Timeout) => (),
        };
    }

    // Tell the system that service has stopped.
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

fn main() -> Result<(), windows_service::Error> {
    // Register generated `ffi_service_main` with the system and start the service, blocking
    // this thread until the service is stopped.
    service_dispatcher::start("warships_replay_poller", ffi_service_main)?;
    Ok(())
}
