use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

const MAX_LOG_LINES: usize = 80;

#[derive(Debug, Clone)]
pub enum WorkerCmd {
    ConnectOut(String),
    ConnectIn(String),
    SendMidi3([u8; 3], u8),
    SendSysex(Vec<u8>),
    SendBatch(Vec<Vec<u8>>),
    SendRawSysex(Vec<Vec<u8>>),
    Shutdown,
}

#[derive(Clone)]
pub struct SharedMidiState {
    pub selected_out: Arc<Mutex<String>>,
    pub selected_in: Arc<Mutex<String>>,
    pub status: Arc<Mutex<String>>,
    pub logs: Arc<Mutex<VecDeque<String>>>,
    pub captured_sysex: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Default for SharedMidiState {
    fn default() -> Self {
        Self {
            selected_out: Arc::new(Mutex::new(String::new())),
            selected_in: Arc::new(Mutex::new(String::new())),
            status: Arc::new(Mutex::new("Disconnected".to_string())),
            logs: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES))),
            captured_sysex: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl SharedMidiState {
    pub fn push_log(&self, line: String) {
        if let Ok(mut logs) = self.logs.lock() {
            if logs.len() >= MAX_LOG_LINES {
                logs.pop_front();
            }
            logs.push_back(line);
        }
    }

    pub fn set_status(&self, msg: impl Into<String>) {
        if let Ok(mut s) = self.status.lock() {
            *s = msg.into();
        }
    }
}

pub fn list_midi_ports() -> (Vec<String>, Vec<String>) {
    let outs = MidiOutput::new("ds330-scan-out")
        .map(|m| {
            m.ports()
                .iter()
                .filter_map(|p| m.port_name(p).ok())
                .collect()
        })
        .unwrap_or_default();

    let ins = MidiInput::new("ds330-scan-in")
        .map(|m| {
            m.ports()
                .iter()
                .filter_map(|p| m.port_name(p).ok())
                .collect()
        })
        .unwrap_or_default();

    (outs, ins)
}

pub struct MidiWorker {
    pub tx: Sender<WorkerCmd>,
    pub state: SharedMidiState,
    handle: Option<JoinHandle<()>>,
}

impl MidiWorker {
    pub fn spawn() -> Self {
        let (tx, rx) = crossbeam_channel::bounded(2048);
        let state = SharedMidiState::default();
        let worker_state = state.clone();

        let handle = thread::Builder::new()
            .name("ds330-midi-worker".to_string())
            .spawn(move || run_worker(rx, worker_state))
            .expect("failed to spawn ds330 midi worker");

        Self {
            tx,
            state,
            handle: Some(handle),
        }
    }

    pub fn try_send(&self, cmd: WorkerCmd) {
        let _ = self.tx.try_send(cmd);
    }
}

impl Drop for MidiWorker {
    fn drop(&mut self) {
        let _ = self.tx.send(WorkerCmd::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn connect_output(port_name: &str, state: &SharedMidiState) -> Option<MidiOutputConnection> {
    let midi_out = MidiOutput::new("DS330-CLAP-Out").ok()?;
    let port = midi_out
        .ports()
        .into_iter()
        .find(|p| midi_out.port_name(p).ok().as_deref() == Some(port_name))?;

    match midi_out.connect(&port, "ds330-midi-out") {
        Ok(conn) => {
            if let Ok(mut sel) = state.selected_out.lock() {
                *sel = port_name.to_string();
            }
            state.set_status(format!("Out: {}", port_name));
            state.push_log(format!("[INFO] Connected MIDI Out: {}", port_name));
            Some(conn)
        }
        Err(e) => {
            state.set_status(format!("Out Err: {}", e));
            state.push_log(format!("[ERR] MIDI Out failed: {}", e));
            None
        }
    }
}

fn connect_input(port_name: &str, state: &SharedMidiState) -> Option<MidiInputConnection<()>> {
    let mut midi_in = MidiInput::new("DS330-CLAP-In").ok()?;
    midi_in.ignore(Ignore::None);
    let port = midi_in
        .ports()
        .into_iter()
        .find(|p| midi_in.port_name(p).ok().as_deref() == Some(port_name))?;

    let cb_state = state.clone();
    match midi_in.connect(
        &port,
        "ds330-midi-in",
        move |_stamp, message, _| {
            if message.first() == Some(&0xF0) {
                let preview = if message.len() > 24 {
                    format!("{} ... ({} bytes)", format_hex(&message[..24]), message.len())
                } else {
                    format_hex(message)
                };
                cb_state.push_log(format!("RX: {}", preview));
                if let Ok(mut cap) = cb_state.captured_sysex.lock() {
                    cap.push(message.to_vec());
                }
            } else if message.first() != Some(&0xFE) && message.first() != Some(&0xF8) {
                cb_state.push_log(format!("RX MIDI: {}", format_hex(message)));
            }
        },
        (),
    ) {
        Ok(conn) => {
            if let Ok(mut sel) = state.selected_in.lock() {
                *sel = port_name.to_string();
            }
            state.push_log(format!("[INFO] Connected MIDI In: {}", port_name));
            Some(conn)
        }
        Err(e) => {
            state.set_status(format!("In Err: {}", e));
            state.push_log(format!("[ERR] MIDI In failed: {}", e));
            None
        }
    }
}

pub fn split_syx_messages(data: &[u8]) -> Vec<Vec<u8>> {
    let mut messages = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0xF0 {
            if let Some(end_rel) = data[i..].iter().position(|&b| b == 0xF7) {
                messages.push(data[i..=i + end_rel].to_vec());
                i += end_rel + 1;
                continue;
            }
        }
        i += 1;
    }
    messages
}

fn format_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn run_worker(rx: Receiver<WorkerCmd>, state: SharedMidiState) {
    let mut out_conn: Option<MidiOutputConnection> = None;
    let mut _in_conn: Option<MidiInputConnection<()>> = None;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            WorkerCmd::ConnectOut(name) => {
                out_conn = None;
                out_conn = connect_output(&name, &state);
            }
            WorkerCmd::ConnectIn(name) => {
                _in_conn = None;
                _in_conn = connect_input(&name, &state);
            }
            WorkerCmd::SendMidi3(bytes, len) => {
                if let Some(conn) = out_conn.as_mut() {
                    let n = (len as usize).min(3);
                    let _ = conn.send(&bytes[..n]);
                }
            }
            WorkerCmd::SendSysex(msg) => {
                if let Some(conn) = out_conn.as_mut() {
                    let _ = conn.send(&msg);
                    state.push_log(format!("TX SysEx: {}", format_hex(&msg)));
                    thread::sleep(Duration::from_millis(25));
                }
            }
            WorkerCmd::SendBatch(msgs) => {
                if let Some(conn) = out_conn.as_mut() {
                    for (i, msg) in msgs.iter().enumerate() {
                        if i > 0 {
                            thread::sleep(Duration::from_millis(20));
                        }
                        let _ = conn.send(msg);
                    }
                    state.push_log(format!("TX Batch ({} msgs)", msgs.len()));
                } else {
                    state.push_log("[WARN] Connect MIDI Out first".to_string());
                }
            }
            WorkerCmd::SendRawSysex(msgs) => {
                if let Some(conn) = out_conn.as_mut() {
                    for (i, msg) in msgs.iter().enumerate() {
                        if i > 0 {
                            thread::sleep(Duration::from_millis(80));
                        }
                        let _ = conn.send(msg);
                        state.push_log(format!("TX: Raw SysEx ({} bytes)", msg.len()));
                    }
                } else {
                    state.push_log("[WARN] Connect MIDI Out before sending .syx".to_string());
                }
            }
            WorkerCmd::Shutdown => break,
        }
    }
}
