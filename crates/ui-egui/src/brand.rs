//! Original text branding for the Ama Bartalap fork. No upstream trademark artwork embedded.
pub fn wordmark(_ctx: &egui::Context, _dark: bool) -> Option<egui::TextureHandle> {
    None
}
pub fn paint_wordmark(ui: &egui::Ui, left_center: egui::Pos2, height: f32, _dark: bool) -> Option<egui::Rect> {
    Some(ui.painter().text(left_center, egui::Align2::LEFT_CENTER, "Ama Bartalap", egui::FontId::proportional(height * 0.7), ui.visuals().text_color()))
}
