use eframe::egui;

#[derive(PartialEq, Clone, Copy)]
pub enum ColorPalette {
    Classic,
    Grayscale,
    GrayscaleDark,
    GrayscaleLight,
    Neon,
    Pastel,
}

impl ColorPalette {
    pub fn label(self) -> &'static str {
        match self {
            ColorPalette::Classic => "Classic",
            ColorPalette::Grayscale => "Grayscale",
            ColorPalette::GrayscaleDark => "Grayscale (Dark)",
            ColorPalette::GrayscaleLight => "Grayscale (Light)",
            ColorPalette::Neon => "Neon",
            ColorPalette::Pastel => "Pastel",
        }
    }
}

const ALL_PALETTES: &[ColorPalette] = &[
    ColorPalette::Classic,
    ColorPalette::Grayscale,
    ColorPalette::GrayscaleDark,
    ColorPalette::GrayscaleLight,
    ColorPalette::Neon,
    ColorPalette::Pastel,
];

pub fn build_palette(palette: ColorPalette, num_states: usize) -> Vec<egui::Color32> {
    let colors: &[egui::Color32] = match palette {
        ColorPalette::Classic => &[
            egui::Color32::BLACK,
            egui::Color32::WHITE,
            egui::Color32::from_rgb(55, 200, 80),
            egui::Color32::from_rgb(255, 105, 30),
            egui::Color32::from_rgb(60, 115, 235),
            egui::Color32::from_rgb(210, 50, 195),
            egui::Color32::from_rgb(235, 195, 30),
            egui::Color32::from_rgb(35, 190, 185),
        ],
        ColorPalette::Grayscale => &[
            egui::Color32::BLACK,
            egui::Color32::WHITE,
            egui::Color32::from_gray(64),
            egui::Color32::from_gray(192),
            egui::Color32::from_gray(128),
            egui::Color32::from_gray(96),
            egui::Color32::from_gray(160),
            egui::Color32::from_gray(224),
        ],
        ColorPalette::GrayscaleDark => &[
            egui::Color32::from_gray(0),
            egui::Color32::from_gray(36),
            egui::Color32::from_gray(72),
            egui::Color32::from_gray(108),
            egui::Color32::from_gray(144),
            egui::Color32::from_gray(180),
            egui::Color32::from_gray(210),
            egui::Color32::from_gray(235),
        ],
        ColorPalette::GrayscaleLight => &[
            egui::Color32::from_gray(255),
            egui::Color32::from_gray(220),
            egui::Color32::from_gray(184),
            egui::Color32::from_gray(148),
            egui::Color32::from_gray(112),
            egui::Color32::from_gray(76),
            egui::Color32::from_gray(45),
            egui::Color32::from_gray(20),
        ],
        ColorPalette::Neon => &[
            egui::Color32::from_rgb(10, 10, 10),
            egui::Color32::from_rgb(0, 255, 100),
            egui::Color32::from_rgb(255, 50, 200),
            egui::Color32::from_rgb(0, 200, 255),
            egui::Color32::from_rgb(255, 200, 0),
            egui::Color32::from_rgb(255, 50, 50),
            egui::Color32::from_rgb(150, 50, 255),
            egui::Color32::WHITE,
        ],
        ColorPalette::Pastel => &[
            egui::Color32::from_rgb(50, 50, 70),
            egui::Color32::from_rgb(255, 200, 210),
            egui::Color32::from_rgb(210, 255, 210),
            egui::Color32::from_rgb(255, 255, 200),
            egui::Color32::from_rgb(200, 220, 255),
            egui::Color32::from_rgb(255, 220, 190),
            egui::Color32::from_rgb(210, 255, 255),
            egui::Color32::from_rgb(240, 210, 255),
        ],
    };
    colors[..num_states.min(colors.len())].to_vec()
}

/// Returns true if `colors` was modified (caller should refresh any rendered textures).
pub fn draw_palette_params(
    ui: &mut egui::Ui,
    selected: &mut ColorPalette,
    colors: &mut Vec<egui::Color32>,
    num_states: usize,
) -> bool {
    let mut changed = false;

    egui::CollapsingHeader::new("Palette")
        .default_open(true)
        .show(ui, |ui| {
            let mut palette_changed = false;
            egui::ComboBox::from_id_salt("palette_select")
                .selected_text(selected.label())
                .show_ui(ui, |ui| {
                    for &p in ALL_PALETTES {
                        if ui.selectable_value(selected, p, p.label()).changed() {
                            palette_changed = true;
                        }
                    }
                });
            if palette_changed {
                *colors = build_palette(*selected, num_states);
                changed = true;
            }
            if ui.button("Cycle state colors").clicked() {
                colors.rotate_left(1);
                changed = true;
            }
        });

    changed
}
