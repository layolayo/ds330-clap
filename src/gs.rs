//! Roland GS / Dr. Synth DS-330 SysEx and GM/GS constants.

use serde::{Deserialize, Serialize};

pub const ROLAND_ID: u8 = 0x41;
pub const GS_MODEL_ID: u8 = 0x42;
pub const GS_DT1_CMD: u8 = 0x12;

/// Standard Roland GS checksum over address + data bytes.
pub fn roland_checksum(addr: [u8; 3], data: &[u8]) -> u8 {
    let sum: u32 = addr.iter().chain(data.iter()).map(|&b| b as u32).sum();
    ((128 - (sum % 128)) & 0x7F) as u8
}

/// Build a Roland GS DT1 SysEx message: `F0 41 <dev> 42 12 <a0> <a1> <a2> <data...> <chk> F7`.
pub fn build_gs_dt1(dev_id: u8, addr: [u8; 3], data: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(9 + data.len());
    msg.extend_from_slice(&[
        0xF0,
        ROLAND_ID,
        dev_id & 0x1F,
        GS_MODEL_ID,
        GS_DT1_CMD,
        addr[0] & 0x7F,
        addr[1] & 0x7F,
        addr[2] & 0x7F,
    ]);
    for &b in data {
        msg.push(b & 0x7F);
    }
    msg.push(roland_checksum(addr, data));
    msg.push(0xF7);
    msg
}

/// GS Reset (`F0 41 <dev> 42 12 40 00 7F 00 <chk> F7`).
pub fn build_gs_reset(dev_id: u8) -> Vec<u8> {
    build_gs_dt1(dev_id, [0x40, 0x00, 0x7F], &[0x00])
}

/// Roland GS Patch Part address middle byte for MIDI channel `0..=15`:
/// Ch 10 (idx 9) -> `0x10`, Ch 1..9 (idx 0..=8) -> `0x11..=0x19`, Ch 11..16 -> `0x1A..=0x1F`.
pub fn gs_part_addr_mid(part_idx: u8) -> u8 {
    match part_idx {
        9 => 0x10,
        0..=8 => 0x11 + part_idx,
        10..=15 => 0x1A + (part_idx - 10),
        _ => 0x11,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartState {
    pub bank_msb: u8,
    pub program: u8,
    pub volume: u8,
    pub pan: u8,
    pub reverb_send: u8,
    pub chorus_send: u8,
    pub cutoff: u8,
    pub resonance: u8,
    pub attack: u8,
    pub decay: u8,
    pub release: u8,
    pub vib_rate: u8,
    pub vib_depth: u8,
    pub vib_delay: u8,
    pub mod_wheel: u8,
    pub porta_on: bool,
    pub porta_time: u8,
    pub drum_mode: u8, // 0 = Normal, 1 = Drum1, 2 = Drum2
    pub muted: bool,
}

impl PartState {
    pub fn new_for_part(part_idx: u8) -> Self {
        Self {
            bank_msb: 0,
            program: 0,
            volume: 100,
            pan: 64,
            reverb_send: 40,
            chorus_send: 0,
            cutoff: 64,
            resonance: 64,
            attack: 64,
            decay: 64,
            release: 64,
            vib_rate: 64,
            vib_depth: 64,
            vib_delay: 64,
            mod_wheel: 0,
            porta_on: false,
            porta_time: 0,
            drum_mode: if part_idx == 9 { 1 } else { 0 },
            muted: false,
        }
    }
}

pub const REVERB_MACROS: [&str; 8] = [
    "Room 1", "Room 2", "Room 3", "Hall 1", "Hall 2", "Plate", "Delay", "Pan Delay",
];

pub const CHORUS_MACROS: [&str; 8] = [
    "Chorus 1",
    "Chorus 2",
    "Chorus 3",
    "Chorus 4",
    "Feedback Cho",
    "Flanger",
    "Short Delay",
    "Short Dly FB",
];

pub const GS_DRUM_KITS: [(u8, &str); 9] = [
    (0, "Standard"),
    (8, "Room"),
    (16, "Power"),
    (24, "Electronic"),
    (25, "TR-808"),
    (32, "Jazz"),
    (40, "Brush"),
    (48, "Orchestra"),
    (56, "SFX"),
];

pub fn gs_drum_kit_name(prog: u8) -> &'static str {
    GS_DRUM_KITS
        .iter()
        .find(|&&(p, _)| p == prog)
        .map(|&(_, n)| n)
        .unwrap_or("Drum Kit")
}

pub fn gm_program_name(prog: u8) -> &'static str {
    const NAMES: [&str; 128] = [
        "Acoustic Grand", "Bright Acoustic", "Electric Grand", "Honky-tonk",
        "Electric Piano 1", "Electric Piano 2", "Harpsichord", "Clavi",
        "Celesta", "Glockenspiel", "Music Box", "Vibraphone",
        "Marimba", "Xylophone", "Tubular Bells", "Dulcimer",
        "Drawbar Organ", "Percussive Organ", "Rock Organ", "Church Organ",
        "Reed Organ", "Accordion", "Harmonica", "Tango Accordion",
        "Nylon Guitar", "Steel Guitar", "Jazz Guitar", "Clean Guitar",
        "Muted Guitar", "Overdriven Gtr", "Distortion Gtr", "Gtr Harmonics",
        "Acoustic Bass", "Finger Bass", "Pick Bass", "Fretless Bass",
        "Slap Bass 1", "Slap Bass 2", "Synth Bass 1", "Synth Bass 2",
        "Violin", "Viola", "Cello", "Contrabass",
        "Tremolo Strings", "Pizzicato Str", "Orchestral Harp", "Timpani",
        "String Ens 1", "String Ens 2", "SynthStrings 1", "SynthStrings 2",
        "Choir Aahs", "Voice Oohs", "Synth Voice", "Orchestra Hit",
        "Trumpet", "Trombone", "Tuba", "Muted Trumpet",
        "French Horn", "Brass Section", "SynthBrass 1", "SynthBrass 2",
        "Soprano Sax", "Alto Sax", "Tenor Sax", "Baritone Sax",
        "Oboe", "English Horn", "Bassoon", "Clarinet",
        "Piccolo", "Flute", "Recorder", "Pan Flute",
        "Blown Bottle", "Shakuhachi", "Whistle", "Ocarina",
        "Lead 1 (Square)", "Lead 2 (Saw)", "Lead 3 (Calliope)", "Lead 4 (Chiff)",
        "Lead 5 (Charang)", "Lead 6 (Voice)", "Lead 7 (Fifths)", "Lead 8 (Bass+Ld)",
        "Pad 1 (New Age)", "Pad 2 (Warm)", "Pad 3 (Polysynth)", "Pad 4 (Choir)",
        "Pad 5 (Bowed)", "Pad 6 (Metallic)", "Pad 7 (Halo)", "Pad 8 (Sweep)",
        "FX 1 (Rain)", "FX 2 (Soundtrack)", "FX 3 (Crystal)", "FX 4 (Atmosphere)",
        "FX 5 (Bright)", "FX 6 (Goblins)", "FX 7 (Echoes)", "FX 8 (Sci-Fi)",
        "Sitar", "Banjo", "Shamisen", "Koto",
        "Kalimba", "Bagpipe", "Fiddle", "Shanai",
        "Tinkle Bell", "Agogo", "Steel Drums", "Woodblock",
        "Taiko Drum", "Melodic Tom", "Synth Drum", "Reverse Cymbal",
        "Gtr Fret Noise", "Breath Noise", "Seashore", "Bird Tweet",
        "Telephone Ring", "Helicopter", "Applause", "Gunshot",
    ];
    NAMES.get(prog as usize).copied().unwrap_or("Unknown")
}
