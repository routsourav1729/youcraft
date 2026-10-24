//! YouCraft branding supplied by the project owner.
pub fn wordmark(ctx: &egui::Context, _dark: bool) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("youcraft-logo");
    if let Some(texture) = ctx.data_mut(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let rgba = image::load_from_memory(include_bytes!("../../../assets/youcraft/logo.png")).ok()?.to_rgba8();
    let image = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
    let texture = ctx.load_texture("YouCraft", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}
pub fn paint_wordmark(ui: &egui::Ui, left_center: egui::Pos2, height: f32, _dark: bool) -> Option<egui::Rect> {
    let mark = egui::Rect::from_min_size(left_center - egui::vec2(0., height / 2.), egui::vec2(height, height));
    if let Some(texture) = wordmark(ui.ctx(), _dark) {
        ui.painter().image(texture.id(), mark, egui::Rect::from_min_max(egui::pos2(0.28, 0.20), egui::pos2(0.72, 0.62)), egui::Color32::WHITE);
    }
    let text = ui.painter().text(
        left_center + egui::vec2(height + 6., 0.),
        egui::Align2::LEFT_CENTER,
        "YouCraft",
        egui::FontId::proportional(height * 0.7),
        ui.visuals().text_color(),
    );
    Some(mark.union(text))
}
