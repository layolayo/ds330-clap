use std::sync::Mutex;
use nih_plug::prelude::*;

use crate::gs::PartState;

#[derive(Params)]
pub struct Ds330Params {
    #[persist = "midi_out"]
    pub saved_midi_out: Mutex<String>,

    #[persist = "midi_in"]
    pub saved_midi_in: Mutex<String>,

    #[persist = "parts_state"]
    pub parts: Mutex<Vec<PartState>>,

    #[id = "dev_id"]
    pub device_id: IntParam,

    #[id = "act_part"]
    pub active_part: IntParam,

    #[id = "route_to_part"]
    pub route_notes_to_active: BoolParam,

    // Active-part automatable controls (forwarded as CCs on active_part)
    #[id = "vol"]
    pub volume: IntParam,
    #[id = "pan"]
    pub pan: IntParam,
    #[id = "rev_send"]
    pub reverb_send: IntParam,
    #[id = "cho_send"]
    pub chorus_send: IntParam,
    #[id = "cutoff"]
    pub cutoff: IntParam,
    #[id = "res"]
    pub resonance: IntParam,
    #[id = "atk"]
    pub attack: IntParam,
    #[id = "dec"]
    pub decay: IntParam,
    #[id = "rel"]
    pub release: IntParam,
    #[id = "vib_rate"]
    pub vib_rate: IntParam,
    #[id = "vib_depth"]
    pub vib_depth: IntParam,
    #[id = "vib_delay"]
    pub vib_delay: IntParam,
    #[id = "mod_wh"]
    pub mod_wheel: IntParam,

    // Global GS Master & FX
    #[id = "m_vol"]
    pub master_vol: IntParam,
    #[id = "m_pan"]
    pub master_pan: IntParam,
    #[id = "m_trans"]
    pub master_transpose: IntParam,

    #[id = "rev_mac"]
    pub rev_macro: IntParam,
    #[id = "rev_lvl"]
    pub rev_level: IntParam,
    #[id = "rev_tim"]
    pub rev_time: IntParam,
    #[id = "rev_fb"]
    pub rev_feedback: IntParam,

    #[id = "cho_mac"]
    pub cho_macro: IntParam,
    #[id = "cho_lvl"]
    pub cho_level: IntParam,
    #[id = "cho_rate"]
    pub cho_rate: IntParam,
    #[id = "cho_dep"]
    pub cho_depth: IntParam,
}

impl Default for Ds330Params {
    fn default() -> Self {
        let parts = (0..16).map(PartState::new_for_part).collect();
        Self {
            saved_midi_out: Mutex::new(String::new()),
            saved_midi_in: Mutex::new(String::new()),
            parts: Mutex::new(parts),
            device_id: IntParam::new("Device ID", 17, IntRange::Linear { min: 1, max: 32 }),
            active_part: IntParam::new("Active Part", 1, IntRange::Linear { min: 1, max: 16 }),
            route_notes_to_active: BoolParam::new("Route Notes to Active Part", false),

            volume: IntParam::new("Part Volume", 100, IntRange::Linear { min: 0, max: 127 }),
            pan: IntParam::new("Part Pan", 64, IntRange::Linear { min: 0, max: 127 }),
            reverb_send: IntParam::new("Reverb Send", 40, IntRange::Linear { min: 0, max: 127 }),
            chorus_send: IntParam::new("Chorus Send", 0, IntRange::Linear { min: 0, max: 127 }),
            cutoff: IntParam::new("TVF Cutoff", 64, IntRange::Linear { min: 0, max: 127 }),
            resonance: IntParam::new("TVF Resonance", 64, IntRange::Linear { min: 0, max: 127 }),
            attack: IntParam::new("TVA Attack", 64, IntRange::Linear { min: 0, max: 127 }),
            decay: IntParam::new("TVA Decay", 64, IntRange::Linear { min: 0, max: 127 }),
            release: IntParam::new("TVA Release", 64, IntRange::Linear { min: 0, max: 127 }),
            vib_rate: IntParam::new("Vibrato Rate", 64, IntRange::Linear { min: 0, max: 127 }),
            vib_depth: IntParam::new("Vibrato Depth", 64, IntRange::Linear { min: 0, max: 127 }),
            vib_delay: IntParam::new("Vibrato Delay", 64, IntRange::Linear { min: 0, max: 127 }),
            mod_wheel: IntParam::new("Mod Wheel", 0, IntRange::Linear { min: 0, max: 127 }),

            master_vol: IntParam::new("Master Volume", 127, IntRange::Linear { min: 0, max: 127 }),
            master_pan: IntParam::new("Master Pan", 64, IntRange::Linear { min: 0, max: 127 }),
            master_transpose: IntParam::new("Master Key Shift", 64, IntRange::Linear { min: 40, max: 88 }),

            rev_macro: IntParam::new("Reverb Type", 4, IntRange::Linear { min: 0, max: 7 }),
            rev_level: IntParam::new("Reverb Level", 64, IntRange::Linear { min: 0, max: 127 }),
            rev_time: IntParam::new("Reverb Time", 64, IntRange::Linear { min: 0, max: 127 }),
            rev_feedback: IntParam::new("Reverb Delay FB", 0, IntRange::Linear { min: 0, max: 127 }),

            cho_macro: IntParam::new("Chorus Type", 2, IntRange::Linear { min: 0, max: 7 }),
            cho_level: IntParam::new("Chorus Level", 64, IntRange::Linear { min: 0, max: 127 }),
            cho_rate: IntParam::new("Chorus Rate", 64, IntRange::Linear { min: 0, max: 127 }),
            cho_depth: IntParam::new("Chorus Depth", 64, IntRange::Linear { min: 0, max: 127 }),
        }
    }
}
