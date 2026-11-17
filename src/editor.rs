use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, RichText};
use nih_plug_egui::{create_egui_editor, EguiState};

use crate::gs::{
    build_gs_dt1, build_gs_reset, gm_program_name, gs_drum_kit_name, gs_part_addr_mid, PartState,
    CHORUS_MACROS, GS_DRUM_KITS, REVERB_MACROS,
};
use crate::midi_worker::{list_midi_ports, split_syx_messages, SharedMidiState, WorkerCmd};
use crate::params::Ds330Params;
use crate::ui_widgets::{draw_tva_graph, draw_tvf_graph, draw_vibrato_graph};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    PartEditor,
    Mixer,
    GlobalFx,
    PatchesSysex,
    MidiLog,
    BitwigHelp,
}

struct EditorUiState {
    tab: Tab,
    out_ports: Vec<String>,
    in_ports: Vec<String>,
    last_loaded_part: i32,
    syx_filename: String,
    syx_files: Vec<String>,
    syx_status: String,
}

impl Default for EditorUiState {
    fn default() -> Self {
        let (out_ports, in_ports) = list_midi_ports();
        let mut st = Self {
            tab: Tab::PartEditor,
            out_ports,
            in_ports,
            last_loaded_part: 1,
            syx_filename: "ds330_setup.syx".to_string(),
            syx_files: Vec::new(),
            syx_status: String::new(),
        };
        st.refresh_syx_list();
        st
    }
}

impl EditorUiState {
    fn patch_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join("DS330_Patches")
    }

    fn refresh_syx_list(&mut self) {
        let dir = Self::patch_dir();
        let _ = fs::create_dir_all(&dir);
        let mut files = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.to_lowercase().ends_with(".syx") {
                        files.push(name.to_string());
                    }
                }
            }
        }
        files.sort();
        self.syx_files = files;
    }
}

pub fn create(
    params: Arc<Ds330Params>,
    editor_state: Arc<EguiState>,
    midi_state: SharedMidiState,
    cmd_tx: crossbeam_channel::Sender<WorkerCmd>,
) -> Option<Box<dyn Editor>> {
    let ui_state = Arc::new(Mutex::new(EditorUiState::default()));

    create_egui_editor(
        editor_state,
        (),
        |_, _| {},
        move |egui_ctx, setter, _state| {
            let mut ui_st = ui_state.lock().unwrap();
            let dev_id = (params.device_id.value() - 1).clamp(0, 31) as u8;
            let cur_part = params.active_part.value();

            // Sync active part's stored values into the DAW parameters when switching parts
            if ui_st.last_loaded_part != cur_part {
                ui_st.last_loaded_part = cur_part;
                load_part_into_params(&params, (cur_part - 1) as usize, setter);
            } else {
                save_params_into_part(&params, (cur_part - 1) as usize);
            }

            let status = midi_state.status.lock().unwrap().clone();
            let status_color = if status.contains("Err") || status == "Disconnected" {
                Color32::from_rgb(245, 130, 110)
            } else {
                Color32::from_rgb(130, 225, 155)
            };

            egui::TopBottomPanel::bottom("ds330_footer").show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("MIDI: {}", status))
                            .small()
                            .color(status_color),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "v{} ({})",
                                env!("CARGO_PKG_VERSION"),
                                env!("DS330_BUILD_TIME")
                            ))
                            .small()
                            .color(Color32::GRAY),
                        );
                    });
                });
            });

            egui::CentralPanel::default().show(egui_ctx, |ui| {
                draw_top_bar(ui, &params, &midi_state, &cmd_tx, &mut ui_st, dev_id, setter);
                ui.separator();

                ui.horizontal(|ui| {
                    ui.selectable_value(&mut ui_st.tab, Tab::PartEditor, "Part Editor (1-16)");
                    ui.selectable_value(&mut ui_st.tab, Tab::Mixer, "16-Part Mixer");
                    ui.selectable_value(&mut ui_st.tab, Tab::GlobalFx, "Global FX & Master");
                    ui.selectable_value(&mut ui_st.tab, Tab::PatchesSysex, "SysEx & Backup (.syx)");
                    ui.selectable_value(&mut ui_st.tab, Tab::MidiLog, "MIDI Log");
                    ui.selectable_value(&mut ui_st.tab, Tab::BitwigHelp, "Bitwig Setup");
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| match ui_st.tab {
                    Tab::PartEditor => draw_part_editor(ui, &params, &cmd_tx, dev_id, setter),
                    Tab::Mixer => draw_mixer_tab(ui, &params, &cmd_tx, dev_id, setter),
                    Tab::GlobalFx => draw_global_fx_tab(ui, &params, setter),
                    Tab::PatchesSysex => {
                        draw_sysex_tab(ui, &params, &midi_state, &cmd_tx, &mut ui_st, dev_id)
                    }
                    Tab::MidiLog => draw_log_tab(ui, &midi_state),
                    Tab::BitwigHelp => draw_bitwig_help_tab(ui),
                });
            });
        },
    )
}

fn draw_top_bar(
    ui: &mut egui::Ui,
    params: &Ds330Params,
    midi_state: &SharedMidiState,
    cmd_tx: &crossbeam_channel::Sender<WorkerCmd>,
    ui_st: &mut EditorUiState,
    dev_id: u8,
    setter: &ParamSetter,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("ROLAND DS-330")
                .strong()
                .color(Color32::from_rgb(100, 220, 180)),
        );

        let sel_out = midi_state.selected_out.lock().unwrap().clone();
        egui::ComboBox::from_id_salt("ds330_midi_out")
            .selected_text(if sel_out.is_empty() {
                "MIDI Out..."
            } else {
                &sel_out
            })
            .width(155.0)
            .show_ui(ui, |ui| {
                for port in &ui_st.out_ports {
                    if ui.selectable_label(sel_out == *port, port).clicked() {
                        if let Ok(mut s) = params.saved_midi_out.lock() {
                            *s = port.clone();
                        }
                        let _ = cmd_tx.send(WorkerCmd::ConnectOut(port.clone()));
                    }
                }
            });

        let sel_in = midi_state.selected_in.lock().unwrap().clone();
        egui::ComboBox::from_id_salt("ds330_midi_in")
            .selected_text(if sel_in.is_empty() {
                "MIDI In..."
            } else {
                &sel_in
            })
            .width(155.0)
            .show_ui(ui, |ui| {
                for port in &ui_st.in_ports {
                    if ui.selectable_label(sel_in == *port, port).clicked() {
                        if let Ok(mut s) = params.saved_midi_in.lock() {
                            *s = port.clone();
                        }
                        let _ = cmd_tx.send(WorkerCmd::ConnectIn(port.clone()));
                    }
                }
            });

        if ui.button("Refresh").clicked() {
            let (outs, ins) = list_midi_ports();
            ui_st.out_ports = outs;
            ui_st.in_ports = ins;
        }

        ui.label("Dev ID:");
        let mut did = params.device_id.value();
        if ui.add(egui::DragValue::new(&mut did).range(1..=32)).changed() {
            setter.begin_set_parameter(&params.device_id);
            setter.set_parameter(&params.device_id, did);
            setter.end_set_parameter(&params.device_id);
        }

        if ui.button("GS Reset").clicked() {
            let _ = cmd_tx.send(WorkerCmd::SendSysex(build_gs_reset(dev_id)));
        }

        if ui.button("Panic").clicked() {
            let msgs: Vec<Vec<u8>> = (0..16u8)
                .map(|c| vec![0xB0 | c, 123, 0])
                .collect();
            let _ = cmd_tx.send(WorkerCmd::SendBatch(msgs));
        }

        bool_row(ui, "Route Notes -> Active Part", &params.route_notes_to_active, setter);
    });
}

fn draw_part_editor(
    ui: &mut egui::Ui,
    params: &Ds330Params,
    cmd_tx: &crossbeam_channel::Sender<WorkerCmd>,
    dev_id: u8,
    setter: &ParamSetter,
) {
    let cur = params.active_part.value();
    ui.horizontal(|ui| {
        ui.label("Part:");
        for p in 1..=16 {
            let lbl = if p == 10 {
                "10(Dr)".to_string()
            } else {
                format!("{}", p)
            };
            if ui.selectable_label(cur == p, lbl).clicked() {
                setter.begin_set_parameter(&params.active_part);
                setter.set_parameter(&params.active_part, p);
                setter.end_set_parameter(&params.active_part);
            }
        }
    });
    ui.separator();

    let idx = (cur - 1).clamp(0, 15) as usize;
    let ch = idx as u8;
    {
        let mut parts = params.parts.lock().unwrap();
        let part = &mut parts[idx];
        ui.horizontal(|ui| {
            ui.label("Bank (CC 0):");
            let mut bank = part.bank_msb;
            if ui.add(egui::DragValue::new(&mut bank).range(0..=127)).changed() {
                part.bank_msb = bank;
                let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xB0 | ch, 0, bank], 3));
                let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xC0 | ch, part.program, 0], 2));
            }

            ui.label("Program:");
            let mut prog = part.program as i32 + 1;
            if ui.add(egui::DragValue::new(&mut prog).range(1..=128)).changed() {
                part.program = (prog - 1) as u8;
                let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xB0 | ch, 0, part.bank_msb], 3));
                let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xC0 | ch, part.program, 0], 2));
            }

            let tone_label = if part.drum_mode != 0 {
                gs_drum_kit_name(part.program)
            } else {
                gm_program_name(part.program)
            };
            ui.label(
                RichText::new(tone_label)
                    .strong()
                    .color(Color32::from_rgb(255, 220, 130)),
            );

            ui.separator();
            ui.label("Mode:");
            for (m_val, m_name) in [(0u8, "Normal"), (1, "Drum1"), (2, "Drum2")] {
                if ui.selectable_label(part.drum_mode == m_val, m_name).clicked() {
                    part.drum_mode = m_val;
                    let addr = [0x40, gs_part_addr_mid(ch), 0x15];
                    let _ = cmd_tx.send(WorkerCmd::SendSysex(build_gs_dt1(dev_id, addr, &[m_val])));
                }
            }
        });

        if part.drum_mode != 0 {
            ui.horizontal(|ui| {
                ui.label("Drum Kits:");
                for &(kit_prog, kit_name) in &GS_DRUM_KITS {
                    if ui
                        .selectable_label(part.program == kit_prog, kit_name)
                        .clicked()
                    {
                        part.program = kit_prog;
                        let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xB0 | ch, 0, 0], 3));
                        let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xC0 | ch, kit_prog, 0], 2));
                    }
                }
            });
        }
    }
    ui.separator();

    ui.columns(3, |cols| {
        cols[0].group(|ui| {
            ui.horizontal(|ui| {
                ui.heading("TVF Filter & TVA Envelope");
                if ui.button("Reset (64)").clicked() {
                    for p in [
                        &params.cutoff,
                        &params.resonance,
                        &params.attack,
                        &params.decay,
                        &params.release,
                    ] {
                        setter.begin_set_parameter(p);
                        setter.set_parameter(p, 64);
                        setter.end_set_parameter(p);
                    }
                }
            });
            draw_tvf_graph(ui, params.cutoff.value(), params.resonance.value());
            int_row(ui, "Cutoff (CC 74)", &params.cutoff, 0..=127, setter);
            int_row(ui, "Resonance (CC 71)", &params.resonance, 0..=127, setter);
            ui.separator();
            draw_tva_graph(
                ui,
                params.attack.value(),
                params.decay.value(),
                params.release.value(),
            );
            int_row(ui, "Attack (CC 73)", &params.attack, 0..=127, setter);
            int_row(ui, "Decay (CC 75)", &params.decay, 0..=127, setter);
            int_row(ui, "Release (CC 72)", &params.release, 0..=127, setter);
        });

        cols[1].group(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Vibrato & Modulation");
                if ui.button("Reset").clicked() {
                    for p in [&params.vib_rate, &params.vib_depth, &params.vib_delay] {
                        setter.begin_set_parameter(p);
                        setter.set_parameter(p, 64);
                        setter.end_set_parameter(p);
                    }
                    setter.begin_set_parameter(&params.mod_wheel);
                    setter.set_parameter(&params.mod_wheel, 0);
                    setter.end_set_parameter(&params.mod_wheel);
                }
            });
            draw_vibrato_graph(
                ui,
                params.vib_rate.value(),
                params.vib_depth.value(),
                params.vib_delay.value(),
                params.mod_wheel.value(),
            );
            int_row(ui, "Vib Rate (CC 76)", &params.vib_rate, 0..=127, setter);
            int_row(ui, "Vib Depth (CC 77)", &params.vib_depth, 0..=127, setter);
            int_row(ui, "Vib Delay (CC 78)", &params.vib_delay, 0..=127, setter);
            ui.separator();
            int_row(ui, "Mod Wheel (CC 01)", &params.mod_wheel, 0..=127, setter);

            let mut parts = params.parts.lock().unwrap();
            let part = &mut parts[idx];
            if ui.checkbox(&mut part.porta_on, "Portamento (CC 65)").changed() {
                let _ = cmd_tx.send(WorkerCmd::SendMidi3(
                    [0xB0 | ch, 65, if part.porta_on { 127 } else { 0 }],
                    3,
                ));
            }
            ui.horizontal(|ui| {
                ui.label("Porta Time (CC 05)");
                if ui
                    .add(egui::Slider::new(&mut part.porta_time, 0..=127))
                    .changed()
                {
                    let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xB0 | ch, 5, part.porta_time], 3));
                }
            });
        });

        cols[2].group(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Part Mix & Sends");
                if ui.button("Reset").clicked() {
                    for (p, v) in [
                        (&params.volume, 100),
                        (&params.pan, 64),
                        (&params.reverb_send, 40),
                        (&params.chorus_send, 0),
                    ] {
                        setter.begin_set_parameter(p);
                        setter.set_parameter(p, v);
                        setter.end_set_parameter(p);
                    }
                }
            });
            int_row(ui, "Volume (CC 07)", &params.volume, 0..=127, setter);
            int_row(ui, "Pan (CC 10)", &params.pan, 0..=127, setter);
            int_row(ui, "Reverb Send (CC 91)", &params.reverb_send, 0..=127, setter);
            int_row(ui, "Chorus Send (CC 93)", &params.chorus_send, 0..=127, setter);
        });
    });
}

fn draw_mixer_tab(
    ui: &mut egui::Ui,
    params: &Ds330Params,
    cmd_tx: &crossbeam_channel::Sender<WorkerCmd>,
    _dev_id: u8,
    setter: &ParamSetter,
) {
    let mut parts = params.parts.lock().unwrap();
    let active_idx = (params.active_part.value() - 1).clamp(0, 15) as usize;

    for row in 0..2 {
        ui.columns(8, |cols| {
            for col in 0..8 {
                let idx = row * 8 + col;
                let ch = idx as u8;
                let part = &mut parts[idx];
                let ui = &mut cols[col];
                ui.group(|ui| {
                    if ui
                        .selectable_label(active_idx == idx, format!("Ch {}", idx + 1))
                        .clicked()
                    {
                        setter.begin_set_parameter(&params.active_part);
                        setter.set_parameter(&params.active_part, (idx + 1) as i32);
                        setter.end_set_parameter(&params.active_part);
                    }
                    ui.label(RichText::new(gm_program_name(part.program)).small());
                    if ui.add(egui::DragValue::new(&mut part.volume).prefix("Vol:").range(0..=127)).changed() {
                        let v = if part.muted { 0 } else { part.volume };
                        let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xB0 | ch, 7, v], 3));
                    }
                    if ui.add(egui::DragValue::new(&mut part.pan).prefix("Pan:").range(0..=127)).changed() {
                        let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xB0 | ch, 10, part.pan], 3));
                    }
                    if ui.add(egui::DragValue::new(&mut part.reverb_send).prefix("Rev:").range(0..=127)).changed() {
                        let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xB0 | ch, 91, part.reverb_send], 3));
                    }
                    if ui.add(egui::DragValue::new(&mut part.chorus_send).prefix("Cho:").range(0..=127)).changed() {
                        let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xB0 | ch, 93, part.chorus_send], 3));
                    }
                    if ui.checkbox(&mut part.muted, "Mute").changed() {
                        let v = if part.muted { 0 } else { part.volume };
                        let _ = cmd_tx.send(WorkerCmd::SendMidi3([0xB0 | ch, 7, v], 3));
                    }
                });
            }
        });
    }
}

fn draw_global_fx_tab(ui: &mut egui::Ui, params: &Ds330Params, setter: &ParamSetter) {
    ui.columns(3, |cols| {
        cols[0].group(|ui| {
            ui.heading("Master (GS SysEx)");
            int_row(ui, "Master Volume", &params.master_vol, 0..=127, setter);
            int_row(ui, "Master Pan", &params.master_pan, 0..=127, setter);
            ui.horizontal(|ui| {
                int_row(ui, "Key Shift", &params.master_transpose, 40..=88, setter);
                let semi = params.master_transpose.value() - 64;
                if ui.button(format!("{:+} st", semi)).clicked() {
                    setter.begin_set_parameter(&params.master_transpose);
                    setter.set_parameter(&params.master_transpose, 64);
                    setter.end_set_parameter(&params.master_transpose);
                }
            });
        });

        cols[1].group(|ui| {
            ui.heading("GS Reverb");
            let rm = (params.rev_macro.value() as usize).min(7);
            ui.label(RichText::new(REVERB_MACROS[rm]).strong());
            int_row(ui, "Macro (0-7)", &params.rev_macro, 0..=7, setter);
            int_row(ui, "Level", &params.rev_level, 0..=127, setter);
            int_row(ui, "Time", &params.rev_time, 0..=127, setter);
            int_row(ui, "Delay Feedback", &params.rev_feedback, 0..=127, setter);
        });

        cols[2].group(|ui| {
            ui.heading("GS Chorus");
            let cm = (params.cho_macro.value() as usize).min(7);
            ui.label(RichText::new(CHORUS_MACROS[cm]).strong());
            int_row(ui, "Macro (0-7)", &params.cho_macro, 0..=7, setter);
            int_row(ui, "Level", &params.cho_level, 0..=127, setter);
            int_row(ui, "Rate", &params.cho_rate, 0..=127, setter);
            int_row(ui, "Depth", &params.cho_depth, 0..=127, setter);
        });
    });
}

fn draw_sysex_tab(
    ui: &mut egui::Ui,
    params: &Ds330Params,
    midi_state: &SharedMidiState,
    cmd_tx: &crossbeam_channel::Sender<WorkerCmd>,
    ui_st: &mut EditorUiState,
    dev_id: u8,
) {
    let dir = EditorUiState::patch_dir();
    ui.label(format!("SysEx Folder: {}", dir.display()));

    ui.horizontal(|ui| {
        ui.label("Filename:");
        let resp =
            ui.add(egui::TextEdit::singleline(&mut ui_st.syx_filename).desired_width(180.0));
        if resp.has_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape))
        {
            resp.surrender_focus();
        }

        if ui.button("Save Global GS .syx").clicked() {
            let msgs: Vec<Vec<u8>> = vec![
                build_gs_dt1(dev_id, [0x40, 0x00, 0x04], &[params.master_vol.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x00, 0x06], &[params.master_pan.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x00, 0x05], &[params.master_transpose.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x01, 0x30], &[params.rev_macro.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x01, 0x33], &[params.rev_level.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x01, 0x34], &[params.rev_time.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x01, 0x35], &[params.rev_feedback.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x01, 0x38], &[params.cho_macro.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x01, 0x3A], &[params.cho_level.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x01, 0x3D], &[params.cho_rate.value() as u8]),
                build_gs_dt1(dev_id, [0x40, 0x01, 0x3E], &[params.cho_depth.value() as u8]),
            ];
            let mut data = Vec::new();
            for m in msgs {
                data.extend_from_slice(&m);
            }
            let path = dir.join(&ui_st.syx_filename);
            ui_st.syx_status = match fs::write(&path, &data) {
                Ok(_) => {
                    ui_st.refresh_syx_list();
                    format!("Saved Global GS SysEx to {}", path.display())
                }
                Err(e) => format!("Save error: {}", e),
            };
        }

        if ui.button("Push All 16 Parts").clicked() {
            let parts = params.parts.lock().unwrap();
            let mut batch = Vec::new();
            for (idx, p) in parts.iter().enumerate() {
                let ch = idx as u8;
                batch.push(build_gs_dt1(dev_id, [0x40, gs_part_addr_mid(ch), 0x15], &[p.drum_mode]));
                batch.push(vec![0xB0 | ch, 0, p.bank_msb]);
                batch.push(vec![0xC0 | ch, p.program]);
                batch.push(vec![0xB0 | ch, 7, if p.muted { 0 } else { p.volume }]);
                batch.push(vec![0xB0 | ch, 10, p.pan]);
                batch.push(vec![0xB0 | ch, 91, p.reverb_send]);
                batch.push(vec![0xB0 | ch, 93, p.chorus_send]);
            }
            let _ = cmd_tx.send(WorkerCmd::SendBatch(batch));
            ui_st.syx_status = "Pushed all 16 parts to DS-330".to_string();
        }

        if ui.button("Refresh List").clicked() {
            ui_st.refresh_syx_list();
        }
    });

    ui.separator();
    let (cap_count, cap_bytes) = {
        let cap = midi_state.captured_sysex.lock().unwrap();
        let bytes: usize = cap.iter().map(|d| d.len()).sum();
        (cap.len(), bytes)
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "SysEx Recorder (Captured: {} msg(s), {} bytes):",
                cap_count, cap_bytes
            ))
            .strong(),
        );
        if ui.button("Clear Capture").clicked() {
            midi_state.captured_sysex.lock().unwrap().clear();
            ui_st.syx_status = "Capture buffer cleared.".to_string();
        }
        if ui.button("Save Capture .syx").clicked() {
            let cap = midi_state.captured_sysex.lock().unwrap();
            if cap.is_empty() {
                ui_st.syx_status = "Nothing captured yet!".to_string();
            } else {
                let mut combined = Vec::new();
                for d in cap.iter() {
                    combined.extend_from_slice(d);
                }
                let path = dir.join(&ui_st.syx_filename);
                ui_st.syx_status = match fs::write(&path, &combined) {
                    Ok(_) => {
                        ui_st.refresh_syx_list();
                        format!("Saved {} msg(s) ({} bytes) to {}", cap.len(), combined.len(), path.display())
                    }
                    Err(e) => format!("Save error: {}", e),
                };
            }
        }
    });

    if !ui_st.syx_status.is_empty() {
        ui.label(RichText::new(&ui_st.syx_status).color(Color32::LIGHT_GREEN));
    }

    ui.separator();
    ui.heading("Saved .syx Files (Click to Send to DS-330)");
    for fname in ui_st.syx_files.clone() {
        if ui.button(&fname).clicked() {
            let path = dir.join(&fname);
            match fs::read(&path) {
                Ok(bytes) => {
                    let msgs = split_syx_messages(&bytes);
                    let count = msgs.len();
                    let _ = cmd_tx.send(WorkerCmd::SendRawSysex(msgs));
                    ui_st.syx_status = format!("Sent {} SysEx msg(s) from {}", count, fname);
                }
                Err(e) => ui_st.syx_status = format!("Read error: {}", e),
            }
        }
    }
}

fn draw_bitwig_help_tab(ui: &mut egui::Ui) {
    ui.heading("Bitwig Multi-Channel Setup (Aide-Memoire)");
    ui.separator();
    ui.group(|ui| {
        ui.label(RichText::new("1. Main DS-330 Track (Audio Return + Plugin)").strong());
        ui.label("• Add Roland DS-330 Controller and select your MIDI Out port in the top bar.");
        ui.label("• Leave 'Route Notes -> Active Part' UNCHECKED for multi-channel playback.");
        ui.label("• Add Bitwig HW Instrument after it with MIDI Out = None and Audio In = your DS-330 audio inputs.");
        ui.label("• Important: Do NOT assign the DS-330 MIDI port in Bitwig Settings -> Controllers or HW Instrument, or Bitwig will lock it.");
    });
    ui.add_space(6.0);
    ui.group(|ui| {
        ui.label(RichText::new("2. Extra MIDI Tracks (Part 2 Bass, Part 10 Drums, etc.)").strong());
        ui.label("• Do NOT add HW Instrument on these tracks.");
        ui.label("• Set each track's Output to: Notes -> Tracks -> DS-330 (or Roland DS-330 Controller).");
        ui.label("• Add Bitwig's Channel Map Note FX on the track (e.g. All -> 2, or All -> 10 for Drums).");
        ui.label("• When playing live from a keyboard, arm/monitor the extra track (not the main DS-330 track).");
    });
    ui.add_space(6.0);
    ui.group(|ui| {
        ui.label(RichText::new("3. Quick Notes").strong());
        ui.label("• Click 'GS Reset' to initialize all 16 parts (Part 10 = Drum1, others = Normal).");
        ui.label("• TVF, TVA, and Vibrato use center = 64 (preset default). Raise Mod Wheel (CC 1) to hear Vibrato.");
    });
}

fn draw_log_tab(ui: &mut egui::Ui, midi_state: &SharedMidiState) {
    let all_text = {
        let logs = midi_state.logs.lock().unwrap();
        logs.iter().cloned().collect::<Vec<_>>().join("\n")
    };
    ui.horizontal(|ui| {
        if ui.button("Copy All").clicked() {
            ui.ctx().copy_text(all_text.clone());
        }
        if ui.button("Clear").clicked() {
            if let Ok(mut logs) = midi_state.logs.lock() {
                logs.clear();
            }
        }
    });
    ui.separator();
    let logs = midi_state.logs.lock().unwrap();
    for line in logs.iter().rev() {
        ui.monospace(line);
    }
}

fn load_part_into_params(params: &Ds330Params, idx: usize, setter: &ParamSetter) {
    let part: PartState = params.parts.lock().unwrap()[idx].clone();
    let pairs: [(&IntParam, u8); 13] = [
        (&params.volume, part.volume),
        (&params.pan, part.pan),
        (&params.reverb_send, part.reverb_send),
        (&params.chorus_send, part.chorus_send),
        (&params.cutoff, part.cutoff),
        (&params.resonance, part.resonance),
        (&params.attack, part.attack),
        (&params.decay, part.decay),
        (&params.release, part.release),
        (&params.vib_rate, part.vib_rate),
        (&params.vib_depth, part.vib_depth),
        (&params.vib_delay, part.vib_delay),
        (&params.mod_wheel, part.mod_wheel),
    ];
    for (p, v) in pairs {
        setter.begin_set_parameter(p);
        setter.set_parameter(p, v as i32);
        setter.end_set_parameter(p);
    }
}

fn save_params_into_part(params: &Ds330Params, idx: usize) {
    let mut parts = params.parts.lock().unwrap();
    let part = &mut parts[idx];
    part.volume = params.volume.value() as u8;
    part.pan = params.pan.value() as u8;
    part.reverb_send = params.reverb_send.value() as u8;
    part.chorus_send = params.chorus_send.value() as u8;
    part.cutoff = params.cutoff.value() as u8;
    part.resonance = params.resonance.value() as u8;
    part.attack = params.attack.value() as u8;
    part.decay = params.decay.value() as u8;
    part.release = params.release.value() as u8;
    part.vib_rate = params.vib_rate.value() as u8;
    part.vib_depth = params.vib_depth.value() as u8;
    part.vib_delay = params.vib_delay.value() as u8;
    part.mod_wheel = params.mod_wheel.value() as u8;
}

fn int_row(
    ui: &mut egui::Ui,
    label: &str,
    param: &IntParam,
    range: std::ops::RangeInclusive<i32>,
    setter: &ParamSetter,
) {
    let mut val = param.value();
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.add(egui::Slider::new(&mut val, range)).changed() {
            setter.begin_set_parameter(param);
            setter.set_parameter(param, val);
            setter.end_set_parameter(param);
        }
    });
}

fn bool_row(ui: &mut egui::Ui, label: &str, param: &BoolParam, setter: &ParamSetter) {
    let mut val = param.value();
    if ui.checkbox(&mut val, label).changed() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, val);
        setter.end_set_parameter(param);
    }
}
