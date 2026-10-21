//! One bounded command queue and one in-flight result keep all OS work off the UI thread.
use super::{refresh_kind, row_from_process, terminate, ProcessError, ProcessIdentity, ProcessRow};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use sysinfo::{Pid, Process, ProcessesToUpdate, System};

enum Command {
    Scan,
    Terminate(ProcessIdentity),
}
pub(crate) enum WorkerReply {
    Snapshot(Vec<ProcessRow>),
    ScanFailed(ProcessError),
    Terminated(Result<(), ProcessError>),
}

pub(crate) struct MonitorWorker {
    commands: SyncSender<Command>,
    results: Receiver<WorkerReply>,
}

impl MonitorWorker {
    #[cfg(test)]
    pub(crate) fn with_test_reply(reply: WorkerReply) -> Self {
        let (commands, _) = mpsc::sync_channel(1);
        let (outgoing, results) = mpsc::channel();
        outgoing
            .send(reply)
            .unwrap_or_else(|_| panic!("test result queue must be open"));
        Self { commands, results }
    }

    pub(crate) fn start() -> Result<Self, ProcessError> {
        let (commands, incoming) = mpsc::sync_channel(1);
        let (outgoing, results) = mpsc::channel();
        std::thread::Builder::new()
            .name("process-monitor".into())
            .spawn(move || {
                let mut system = System::new();
                let mut sampled = false;
                while let Ok(command) = incoming.recv() {
                    let reply = match command {
                        Command::Scan => {
                            let refreshed = sample_processes(
                                &mut sampled,
                                || {
                                    system.refresh_processes_specifics(
                                        ProcessesToUpdate::All,
                                        refresh_kind(),
                                    )
                                },
                                || std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL),
                            );
                            if let Err(error) = refreshed {
                                if outgoing.send(WorkerReply::ScanFailed(error)).is_err() {
                                    break;
                                }
                                continue;
                            }
                            let owner = system
                                .process(Pid::from_u32(std::process::id()))
                                .and_then(Process::user_id);
                            let cpu_count =
                                std::thread::available_parallelism().map_or(1, usize::from);
                            let rows = system
                                .processes()
                                .values()
                                .map(|process| row_from_process(process, owner, cpu_count))
                                .collect();
                            WorkerReply::Snapshot(rows)
                        }
                        Command::Terminate(identity) => {
                            WorkerReply::Terminated(terminate(&identity))
                        }
                    };
                    if outgoing.send(reply).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| ProcessError::WorkerUnavailable)?;
        Ok(Self { commands, results })
    }

    pub(crate) fn scan(&self) -> Result<(), ProcessError> {
        self.commands
            .try_send(Command::Scan)
            .map_err(|_| ProcessError::WorkerUnavailable)
    }

    pub(crate) fn terminate(&self, identity: ProcessIdentity) -> Result<(), ProcessError> {
        self.commands
            .try_send(Command::Terminate(identity))
            .map_err(|_| ProcessError::WorkerUnavailable)
    }

    pub(crate) fn poll(&self) -> Result<Option<WorkerReply>, ProcessError> {
        match self.results.try_recv() {
            Ok(reply) => Ok(Some(reply)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(ProcessError::WorkerUnavailable),
        }
    }
}

/// A failed enumeration can leave cached rows in sysinfo; never publish those as fresh.
fn sample_processes(
    sampled: &mut bool,
    mut refresh: impl FnMut() -> usize,
    warmup: impl FnOnce(),
) -> Result<(), ProcessError> {
    if !*sampled {
        if refresh() == 0 {
            return Err(ProcessError::ScanFailed);
        }
        warmup();
    }
    if refresh() == 0 {
        *sampled = false;
        return Err(ProcessError::ScanFailed);
    }
    *sampled = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_monitor_scan_rejects_warmup_and_second_refresh_failures() {
        for counts in [vec![0], vec![2, 0]] {
            let mut counts = counts.into_iter();
            let mut sampled = false;
            assert_eq!(
                sample_processes(&mut sampled, || counts.next().unwrap(), || {}),
                Err(ProcessError::ScanFailed)
            );
            assert!(!sampled);
        }
        let mut sampled = true;
        assert_eq!(
            sample_processes(&mut sampled, || 0, || panic!("unexpected warmup")),
            Err(ProcessError::ScanFailed)
        );
        assert!(!sampled);
        assert_eq!(sample_processes(&mut sampled, || 2, || {}), Ok(()));
        assert!(sampled);
    }
}
