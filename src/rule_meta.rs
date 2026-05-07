use eframe::egui;
use crate::simulation::noise_from_slider;

const MAX_RULES: u64 = 10_000;

pub fn max_num_states(half_width: usize) -> usize {
    let width = (2 * half_width + 1) as u32;
    (2..=8).rev().find(|&k| (k as u64).pow(width) <= MAX_RULES).unwrap_or(2)
}

pub fn max_half_width(num_states: usize) -> usize {
    (1..=4).rev().find(|&hw| {
        (num_states as u64).pow((2 * hw + 1) as u32) <= MAX_RULES
    }).unwrap_or(1)
}

#[derive(Default)]
pub struct MetaParamResponse {
    pub num_states_changed: bool,
    pub half_width_changed: bool,
    pub noise_changed: bool,
}

pub fn draw_rule_meta_params(
    ui: &mut egui::Ui,
    num_states: &mut usize,
    half_width: &mut usize,
    noise: &mut f64,
    states_editable: bool,
) -> MetaParamResponse {
    let mut resp = MetaParamResponse::default();

    egui::CollapsingHeader::new("Rule Meta-Parameters")
        .default_open(true)
        .show(ui, |ui| {
            ui.label("States:");
            let max_k = max_num_states(*half_width);
            if ui
                .add_enabled(
                    states_editable,
                    egui::Slider::new(num_states, 2..=max_k).integer(),
                )
                .changed()
            {
                resp.num_states_changed = true;
            }

            ui.label("Rule width:");
            let max_hw = max_half_width(*num_states);
            if ui
                .add_enabled(
                    states_editable,
                    egui::Slider::new(half_width, 1..=max_hw)
                        .integer()
                        .custom_formatter(|v, _| format!("{} cells", 2 * v as usize + 1)),
                )
                .changed()
            {
                resp.half_width_changed = true;
            }

            let rule_count = (*num_states as u64).pow((2 * *half_width + 1) as u32);
            ui.label(
                egui::RichText::new(format!("{} rules", rule_count))
                    .small()
                    .color(egui::Color32::GRAY),
            );

            ui.label("Noise:");
            let mut noise_t: f64 = if *noise > 0.0 {
                ((*noise).log10() + 7.0) / 6.0
            } else {
                0.0
            };
            noise_t = noise_t.clamp(0.0, 1.0);
            if ui
                .add(
                    egui::Slider::new(&mut noise_t, 0.0f64..=1.0)
                        .custom_formatter(|v, _| {
                            let n = noise_from_slider(v);
                            if n == 0.0 {
                                "0".to_string()
                            } else {
                                format!("{:.2e}", n)
                            }
                        })
                        .custom_parser(|s| {
                            s.parse::<f64>().ok().map(|noise_val| {
                                if noise_val > 0.0 {
                                    ((noise_val.log10() + 7.0) / 6.0).clamp(0.0, 1.0)
                                } else {
                                    0.0
                                }
                            })
                        }),
                )
                .changed()
            {
                *noise = noise_from_slider(noise_t);
                resp.noise_changed = true;
            }
        });

    resp
}
