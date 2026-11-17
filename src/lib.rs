use std::num::NonZeroU32;
use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;

mod editor;
mod gs;
mod midi_worker;
mod params;
mod ui_widgets;

use gs::build_gs_dt1;
use midi_worker::{MidiWorker, WorkerCmd};
use params::Ds330Params;

pub struct Ds330Plugin {
    params: Arc<Ds330Params>,
    editor_state: Arc<EguiState>,
    midi_worker: MidiWorker,
    last_cc: [i32; 13],
    last_gs: [i32; 11],
    last_active_part: i32,
    shadows_initialized: bool,
}

impl Default for Ds330Plugin {
    fn default() -> Self {
        Self {
            params: Arc::new(Ds330Params::default()),
            editor_state: EguiState::from_size(960, 560),
            midi_worker: MidiWorker::spawn(),
            last_cc: [-1; 13],
            last_gs: [-1; 11],
            last_active_part: -1,
            shadows_initialized: false,
        }
    }
}

impl Ds330Plugin {
    fn sync_params(&mut self) {
        let part = self.params.active_part.value().clamp(1, 16);
        let ch = (part - 1) as u8;
        let dev_id = (self.params.device_id.value() - 1).clamp(0, 31) as u8;

        // Direct MIDI CCs (Volume, Pan, Reverb, Chorus, Mod Wheel)
        let cc_vals: [(u8, i32); 5] = [
            (7, self.params.volume.value()),
            (10, self.params.pan.value()),
            (91, self.params.reverb_send.value()),
            (93, self.params.chorus_send.value()),
            (1, self.params.mod_wheel.value()),
        ];

        // Roland GS NRPNs (CC 99=1, CC 98=<lsb>, CC 6=<val>) for TVF/TVA/Vibrato on DS-330
        let nrpn_vals: [(u8, i32); 8] = [
            (0x20, self.params.cutoff.value()),
            (0x21, self.params.resonance.value()),
            (0x63, self.params.attack.value()),
            (0x64, self.params.decay.value()),
            (0x66, self.params.release.value()),
            (0x08, self.params.vib_rate.value()),
            (0x09, self.params.vib_depth.value()),
            (0x0A, self.params.vib_delay.value()),
        ];

        // Avoid sending stale CCs/NRPNs right on the frame active_part switches
        let part_changed = self.last_active_part != part;
        self.last_active_part = part;

        for (i, &(cc, val)) in cc_vals.iter().enumerate() {
            if self.last_cc[i] != val {
                self.last_cc[i] = val;
                if self.shadows_initialized && !part_changed {
                    self.midi_worker
                        .try_send(WorkerCmd::SendMidi3([0xB0 | ch, cc, val as u8], 3));
                }
            }
        }

        for (i, &(nrpn_lsb, val)) in nrpn_vals.iter().enumerate() {
            let idx = 5 + i;
            if self.last_cc[idx] != val {
                self.last_cc[idx] = val;
                if self.shadows_initialized && !part_changed {
                    self.midi_worker
                        .try_send(WorkerCmd::SendMidi3([0xB0 | ch, 99, 1], 3));
                    self.midi_worker
                        .try_send(WorkerCmd::SendMidi3([0xB0 | ch, 98, nrpn_lsb], 3));
                    self.midi_worker
                        .try_send(WorkerCmd::SendMidi3([0xB0 | ch, 6, val as u8], 3));
                }
            }
        }

        let gs_vals: [([u8; 3], i32); 11] = [
            ([0x40, 0x00, 0x04], self.params.master_vol.value()),
            ([0x40, 0x00, 0x06], self.params.master_pan.value()),
            ([0x40, 0x00, 0x05], self.params.master_transpose.value()),
            ([0x40, 0x01, 0x30], self.params.rev_macro.value()),
            ([0x40, 0x01, 0x33], self.params.rev_level.value()),
            ([0x40, 0x01, 0x34], self.params.rev_time.value()),
            ([0x40, 0x01, 0x35], self.params.rev_feedback.value()),
            ([0x40, 0x01, 0x38], self.params.cho_macro.value()),
            ([0x40, 0x01, 0x3A], self.params.cho_level.value()),
            ([0x40, 0x01, 0x3D], self.params.cho_rate.value()),
            ([0x40, 0x01, 0x3E], self.params.cho_depth.value()),
        ];

        for (i, &(addr, val)) in gs_vals.iter().enumerate() {
            if self.last_gs[i] != val {
                self.last_gs[i] = val;
                if self.shadows_initialized {
                    self.midi_worker
                        .try_send(WorkerCmd::SendSysex(build_gs_dt1(dev_id, addr, &[val as u8])));
                }
            }
        }

        self.shadows_initialized = true;
    }
}

impl Plugin for Ds330Plugin {
    const NAME: &'static str = "Roland DS-330 Controller";
    const VENDOR: &'static str = "DS330-CLAP";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.editor_state.clone(),
            self.midi_worker.state.clone(),
            self.midi_worker.tx.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        if let Ok(out) = self.params.saved_midi_out.lock() {
            if !out.is_empty() {
                self.midi_worker.try_send(WorkerCmd::ConnectOut(out.clone()));
            }
        }
        if let Ok(inp) = self.params.saved_midi_in.lock() {
            if !inp.is_empty() {
                self.midi_worker.try_send(WorkerCmd::ConnectIn(inp.clone()));
            }
        }
        self.sync_params();
        true
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let route_active = self.params.route_notes_to_active.value();
        let active_ch = (self.params.active_part.value() - 1).clamp(0, 15) as u8;
        let pick_ch = |in_ch: u8| if route_active { active_ch } else { in_ch & 0x0F };

        while let Some(event) = context.next_event() {
            match event {
                NoteEvent::NoteOn {
                    channel,
                    note,
                    velocity,
                    ..
                } => {
                    let vel = (velocity * 127.0).round().clamp(1.0, 127.0) as u8;
                    self.midi_worker.try_send(WorkerCmd::SendMidi3(
                        [0x90 | pick_ch(channel), note & 0x7F, vel],
                        3,
                    ));
                }
                NoteEvent::NoteOff {
                    channel,
                    note,
                    velocity,
                    ..
                } => {
                    let vel = (velocity * 127.0).round().clamp(0.0, 127.0) as u8;
                    self.midi_worker.try_send(WorkerCmd::SendMidi3(
                        [0x80 | pick_ch(channel), note & 0x7F, vel],
                        3,
                    ));
                }
                NoteEvent::MidiCC {
                    channel, cc, value, ..
                } => {
                    let val = (value * 127.0).round().clamp(0.0, 127.0) as u8;
                    self.midi_worker.try_send(WorkerCmd::SendMidi3(
                        [0xB0 | pick_ch(channel), cc & 0x7F, val],
                        3,
                    ));
                }
                NoteEvent::MidiPitchBend { channel, value, .. } => {
                    let bend = (value * 16383.0).round().clamp(0.0, 16383.0) as u16;
                    self.midi_worker.try_send(WorkerCmd::SendMidi3(
                        [
                            0xE0 | pick_ch(channel),
                            (bend & 0x7F) as u8,
                            ((bend >> 7) & 0x7F) as u8,
                        ],
                        3,
                    ));
                }
                NoteEvent::MidiProgramChange {
                    channel, program, ..
                } => {
                    self.midi_worker.try_send(WorkerCmd::SendMidi3(
                        [0xC0 | pick_ch(channel), program & 0x7F, 0],
                        2,
                    ));
                }
                NoteEvent::MidiChannelPressure {
                    channel, pressure, ..
                } => {
                    let val = (pressure * 127.0).round().clamp(0.0, 127.0) as u8;
                    self.midi_worker.try_send(WorkerCmd::SendMidi3(
                        [0xD0 | pick_ch(channel), val, 0],
                        2,
                    ));
                }
                _ => {}
            }
            context.send_event(event);
        }

        self.sync_params();
        ProcessStatus::Normal
    }
}

impl ClapPlugin for Ds330Plugin {
    const CLAP_ID: &'static str = "org.roland.ds330-editor";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Linux CLAP controller for the Roland Dr. Synth DS-330");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

nih_export_clap!(Ds330Plugin);
