use eframe::egui;

pub fn tex_options() -> egui::TextureOptions {
    egui::TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Linear,
        mipmap_mode: Some(egui::TextureFilter::Linear),
        ..Default::default()
    }
}

pub fn cells_to_color_image(
    cells: &[u8],
    width: usize,
    height: usize,
    palette: &[egui::Color32],
) -> egui::ColorImage {
    let pixels = cells.iter()
        .map(|&v| palette[v as usize % palette.len()])
        .collect();
    egui::ColorImage { size: [width, height], pixels }
}

pub fn make_sim_texture(
    ctx: &egui::Context,
    name: &str,
    cells: &[u8],
    width: usize,
    height: usize,
    palette: &[egui::Color32],
) -> egui::TextureHandle {
    ctx.load_texture(name, cells_to_color_image(cells, width, height, palette), tex_options())
}
