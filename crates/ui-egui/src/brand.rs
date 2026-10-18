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
    Some(ui.painter().text(left_center, egui::Align2::LEFT_CENTER, "YouCraft", egui::FontId::proportional(height * 0.7), ui.visuals().text_color()))
}
