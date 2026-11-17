use nih_plug_egui::egui::{self, Color32, Pos2, Stroke, Vec2};

/// Draw a live TVF Low-Pass Filter frequency response curve (Cutoff + Resonance).
pub fn draw_tvf_graph(ui: &mut egui::Ui, cutoff: i32, resonance: i32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0_f32, Color32::from_rgb(18, 22, 28));

    let pad = 4.0_f32;
    let w = (rect.width() - pad * 2.0_f32).max(10.0_f32);
    let h = (rect.height() - pad * 2.0_f32).max(10.0_f32);
    let left = rect.left() + pad;
    let bottom = rect.bottom() - pad;

    let cut_norm = (cutoff as f32 / 127.0_f32).clamp(0.08, 0.92);
    let res_norm = (resonance as f32 / 127.0_f32).clamp(0.0, 1.0);
    let base_y = bottom - h * 0.55_f32;
    let peak_h = res_norm * h * 0.42_f32;

    let steps = 48;
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let x_norm = i as f32 / steps as f32;
        let dist = x_norm - cut_norm;
        // Smooth low-pass rolloff + resonant bump near cutoff
        let pass = 1.0_f32 / (1.0_f32 + (dist * 10.0_f32).exp());
        let bump = (-((dist * 14.0_f32).powi(2))).exp() * peak_h;
        let y = (bottom - pass * (bottom - base_y) - bump).clamp(rect.top() + 2.0, bottom);
        pts.push(Pos2::new(left + x_norm * w, y));
    }

    p.add(egui::Shape::line(
        pts,
        Stroke::new(1.8_f32, Color32::from_rgb(90, 215, 250)),
    ));
}

/// Draw a live TVA Envelope curve (Attack, Decay, Release relative to GS neutral 64).
pub fn draw_tva_graph(ui: &mut egui::Ui, attack: i32, decay: i32, release: i32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0_f32, Color32::from_rgb(18, 22, 28));

    let pad = 4.0_f32;
    let w = (rect.width() - pad * 2.0_f32).max(10.0_f32);
    let h = (rect.height() - pad * 2.0_f32).max(10.0_f32);
    let left = rect.left() + pad;
    let bottom = rect.bottom() - pad;

    let atk_w = (attack as f32 / 127.0_f32).max(0.03) * (w * 0.30_f32);
    let dec_w = (decay as f32 / 127.0_f32).max(0.05) * (w * 0.28_f32);
    let sus_y = bottom - h * 0.55_f32;
    let rel_w = (release as f32 / 127.0_f32).max(0.05) * (w * 0.28_f32);
    let sus_end = (left + w - rel_w).max(left + atk_w + dec_w + 4.0);

    let pts = [
        Pos2::new(left, bottom),
        Pos2::new(left + atk_w, bottom - h),
        Pos2::new(left + atk_w + dec_w, sus_y),
        Pos2::new(sus_end, sus_y),
        Pos2::new(sus_end + rel_w, bottom),
    ];

    p.add(egui::Shape::line(
        pts.to_vec(),
        Stroke::new(1.8_f32, Color32::from_rgb(130, 230, 180)),
    ));
}

/// Draw a live Vibrato LFO curve (Rate, Depth, Delay, and Mod Wheel).
pub fn draw_vibrato_graph(ui: &mut egui::Ui, rate: i32, depth: i32, delay: i32, mod_wheel: i32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0_f32, Color32::from_rgb(18, 22, 28));

    let cy = rect.center().y;
    let amp = rect.height() * 0.38_f32;
    let cycles = 1.5_f32 + (rate as f32 / 127.0_f32) * 6.0_f32;
    let delay_frac = (delay as f32 / 127.0_f32) * 0.45_f32;
    let depth_norm = (depth as f32 / 127.0_f32) * (0.20_f32 + 0.80_f32 * (mod_wheel as f32 / 127.0_f32));

    let steps = 80;
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let env = if delay == 0 {
            1.0_f32
        } else if t < delay_frac {
            0.0_f32
        } else {
            ((t - delay_frac) / (delay_frac * 0.8_f32).max(0.12_f32)).min(1.0_f32)
        };
        let s = (t * cycles * std::f32::consts::TAU).sin();
        pts.push(Pos2::new(
            rect.left() + 4.0_f32 + t * (rect.width() - 8.0_f32),
            cy - s * env * depth_norm * amp,
        ));
    }

    p.add(egui::Shape::line(
        pts,
        Stroke::new(1.7_f32, Color32::from_rgb(245, 175, 85)),
    ));
}
