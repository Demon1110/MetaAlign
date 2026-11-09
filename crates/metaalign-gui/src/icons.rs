//! Hand drawn vector icons, so the app needs no image assets.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Files,
    Clock,
    Document,
    Play,
    Settings,
    Folder,
    Plus,
    Trash,
    Refresh,
    Sun,
    Moon,
    Info,
    Check,
    Warning,
    Search,
    Dice,
    Export,
    Import,
}

fn inset(rect: Rect, factor: f32) -> Rect {
    Rect::from_center_size(rect.center(), rect.size() * factor)
}

/// Paint an icon inside `rect`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let r = inset(rect, 0.82);
    let stroke = Stroke::new(1.6, color);
    let thin = Stroke::new(1.2, color);
    match icon {
        Icon::Files => {
            let back = Rect::from_min_max(
                r.min + Vec2::new(2.0, 0.0),
                Pos2::new(r.max.x, r.max.y - 3.0),
            );
            painter.rect_filled(back, 3.0, color.gamma_multiply(0.25));
            painter.rect_stroke(back, 3.0, thin, egui::StrokeKind::Inside);
            let front = Rect::from_min_max(
                r.min + Vec2::new(0.0, 3.0),
                Pos2::new(r.max.x - 2.0, r.max.y),
            );
            painter.rect_filled(front, 3.0, color.gamma_multiply(0.45));
            painter.rect_stroke(front, 3.0, stroke, egui::StrokeKind::Inside);
        }
        Icon::Clock => {
            painter.circle_stroke(r.center(), r.width() * 0.42, stroke);
            let center = r.center();
            painter.line_segment(
                [center, center + Vec2::new(0.0, -r.height() * 0.26)],
                stroke,
            );
            painter.line_segment([center, center + Vec2::new(r.width() * 0.22, 0.0)], stroke);
        }
        Icon::Document => {
            painter.rect_stroke(r, 3.0, stroke, egui::StrokeKind::Inside);
            for index in 0..3 {
                let y = r.top() + r.height() * (0.35 + index as f32 * 0.18);
                painter.line_segment(
                    [
                        Pos2::new(r.left() + r.width() * 0.2, y),
                        Pos2::new(r.right() - r.width() * 0.2, y),
                    ],
                    thin,
                );
            }
        }
        Icon::Play => {
            let points = vec![
                Pos2::new(r.left() + r.width() * 0.18, r.top() + r.height() * 0.1),
                Pos2::new(r.right() - r.width() * 0.05, r.center().y),
                Pos2::new(r.left() + r.width() * 0.18, r.bottom() - r.height() * 0.1),
            ];
            painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
        }
        Icon::Settings => {
            painter.circle_stroke(r.center(), r.width() * 0.22, stroke);
            painter.circle_stroke(r.center(), r.width() * 0.42, thin);
            for index in 0..8 {
                let angle = index as f32 * std::f32::consts::TAU / 8.0;
                let direction = Vec2::new(angle.cos(), angle.sin());
                painter.line_segment(
                    [
                        r.center() + direction * r.width() * 0.42,
                        r.center() + direction * r.width() * 0.52,
                    ],
                    thin,
                );
            }
        }
        Icon::Folder => {
            let tab = Rect::from_min_size(
                Pos2::new(r.left(), r.top() + r.height() * 0.16),
                Vec2::new(r.width() * 0.45, r.height() * 0.16),
            );
            painter.rect_filled(tab, 2.0, color.gamma_multiply(0.5));
            let body = Rect::from_min_max(
                Pos2::new(r.left(), r.top() + r.height() * 0.28),
                Pos2::new(r.right(), r.bottom() - r.height() * 0.1),
            );
            painter.rect_filled(body, 3.0, color.gamma_multiply(0.16));
            painter.rect_stroke(body, 3.0, thin, egui::StrokeKind::Inside);
        }
        Icon::Plus => {
            painter.line_segment(
                [
                    Pos2::new(r.center().x, r.top()),
                    Pos2::new(r.center().x, r.bottom()),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(r.left(), r.center().y),
                    Pos2::new(r.right(), r.center().y),
                ],
                stroke,
            );
        }
        Icon::Trash => {
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(r.left() + r.width() * 0.22, r.top() + r.height() * 0.24),
                    Pos2::new(r.right() - r.width() * 0.22, r.bottom()),
                ),
                2.0,
                thin,
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    Pos2::new(r.left() + r.width() * 0.12, r.top() + r.height() * 0.22),
                    Pos2::new(r.right() - r.width() * 0.12, r.top() + r.height() * 0.22),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(r.center().x - r.width() * 0.1, r.top() + r.height() * 0.12),
                    Pos2::new(r.center().x + r.width() * 0.1, r.top() + r.height() * 0.12),
                ],
                thin,
            );
        }
        Icon::Refresh => {
            // Three-quarter arc with a small arrow head, the usual "reload" glyph.
            let radius = r.width() * 0.34;
            let center = r.center();
            let start = -std::f32::consts::FRAC_PI_2;
            let sweep = std::f32::consts::TAU * 0.78;
            let steps = 20;
            let mut previous: Option<Pos2> = None;
            for step in 0..=steps {
                let angle = start + sweep * (step as f32 / steps as f32);
                let point = center + Vec2::new(angle.cos() * radius, angle.sin() * radius);
                if let Some(from) = previous {
                    painter.line_segment([from, point], stroke);
                }
                previous = Some(point);
            }
            let tip = center + Vec2::new(start.cos() * radius, start.sin() * radius);
            let arm = r.width() * 0.17;
            painter.line_segment([tip, tip + Vec2::new(-arm, -arm * 0.5)], stroke);
            painter.line_segment([tip, tip + Vec2::new(arm * 0.4, -arm)], stroke);
        }
        Icon::Sun => {
            painter.circle_stroke(r.center(), r.width() * 0.22, stroke);
            for index in 0..8 {
                let angle = index as f32 * std::f32::consts::TAU / 8.0;
                let direction = Vec2::new(angle.cos(), angle.sin());
                painter.line_segment(
                    [
                        r.center() + direction * r.width() * 0.32,
                        r.center() + direction * r.width() * 0.46,
                    ],
                    thin,
                );
            }
        }
        Icon::Moon => {
            // Outline crescent: an outer arc joined to the arc of an offset circle.
            let radius = r.width() * 0.4;
            let offset = radius * 0.62;
            let cut = radius;
            let x = (offset * offset + radius * radius - cut * cut) / (2.0 * offset);
            let y = (radius * radius - x * x).max(0.0).sqrt();
            let start = y.atan2(x);
            let steps = 16;
            let mut previous: Option<Pos2> = None;

            for step in 0..=steps {
                let angle =
                    start + (std::f32::consts::TAU - 2.0 * start) * (step as f32 / steps as f32);
                let point = r.center() + Vec2::new(angle.cos() * radius, angle.sin() * radius);
                if let Some(from) = previous {
                    painter.line_segment([from, point], thin);
                }
                previous = Some(point);
            }

            let inner_center = r.center() + Vec2::new(offset, 0.0);
            let inner_start = (-y).atan2(x - offset);
            for step in 0..=steps {
                let angle = inner_start + (-2.0 * inner_start) * (step as f32 / steps as f32);
                let point = inner_center + Vec2::new(angle.cos() * cut, angle.sin() * cut);
                if let Some(from) = previous {
                    painter.line_segment([from, point], thin);
                }
                previous = Some(point);
            }
        }
        Icon::Info => {
            painter.circle_stroke(r.center(), r.width() * 0.4, thin);
            painter.text(
                r.center() + Vec2::new(0.0, -r.height() * 0.02),
                egui::Align2::CENTER_CENTER,
                "i",
                egui::FontId::proportional(r.height() * 0.55),
                color,
            );
        }
        Icon::Check => {
            let points = [
                Pos2::new(r.left() + r.width() * 0.1, r.center().y),
                Pos2::new(
                    r.center().x - r.width() * 0.05,
                    r.bottom() - r.height() * 0.15,
                ),
                Pos2::new(r.right() - r.width() * 0.05, r.top() + r.height() * 0.18),
            ];
            painter.line_segment([points[0], points[1]], stroke);
            painter.line_segment([points[1], points[2]], stroke);
        }
        Icon::Warning => {
            let points = vec![
                Pos2::new(r.center().x, r.top() + r.height() * 0.1),
                Pos2::new(r.right() - r.width() * 0.06, r.bottom() - r.height() * 0.12),
                Pos2::new(r.left() + r.width() * 0.06, r.bottom() - r.height() * 0.12),
            ];
            painter.add(Shape::convex_polygon(
                points,
                color.gamma_multiply(0.18),
                thin,
            ));
            painter.text(
                Pos2::new(r.center().x, r.center().y + r.height() * 0.06),
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(r.height() * 0.5),
                color,
            );
        }
        Icon::Search => {
            painter.circle_stroke(
                r.center() + Vec2::new(-r.width() * 0.06, -r.height() * 0.06),
                r.width() * 0.26,
                thin,
            );
            painter.line_segment(
                [
                    r.center() + Vec2::new(r.width() * 0.1, r.height() * 0.1),
                    r.center() + Vec2::new(r.width() * 0.36, r.height() * 0.36),
                ],
                stroke,
            );
        }
        Icon::Dice => {
            painter.rect_stroke(r, 4.0, thin, egui::StrokeKind::Inside);
            for (dx, dy) in [
                (-0.22, -0.22),
                (0.22, -0.22),
                (0.0, 0.0),
                (-0.22, 0.22),
                (0.22, 0.22),
            ] {
                painter.circle_filled(
                    r.center() + Vec2::new(r.width() * dx, r.height() * dy),
                    r.width() * 0.07,
                    color,
                );
            }
        }
        Icon::Export => {
            painter.line_segment(
                [
                    Pos2::new(r.center().x, r.bottom()),
                    Pos2::new(r.center().x, r.top() + r.height() * 0.2),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(r.center().x, r.top() + r.height() * 0.2),
                    Pos2::new(r.left() + r.width() * 0.22, r.top() + r.height() * 0.42),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(r.center().x, r.top() + r.height() * 0.2),
                    Pos2::new(r.right() - r.width() * 0.22, r.top() + r.height() * 0.42),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(r.left(), r.bottom()),
                    Pos2::new(r.right(), r.bottom()),
                ],
                thin,
            );
        }
        Icon::Import => {
            painter.line_segment(
                [
                    Pos2::new(r.center().x, r.top()),
                    Pos2::new(r.center().x, r.bottom() - r.height() * 0.2),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(r.center().x, r.bottom() - r.height() * 0.2),
                    Pos2::new(r.left() + r.width() * 0.22, r.bottom() - r.height() * 0.42),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(r.center().x, r.bottom() - r.height() * 0.2),
                    Pos2::new(r.right() - r.width() * 0.22, r.bottom() - r.height() * 0.42),
                ],
                thin,
            );
            painter.line_segment(
                [Pos2::new(r.left(), r.top()), Pos2::new(r.right(), r.top())],
                thin,
            );
        }
    }
}
