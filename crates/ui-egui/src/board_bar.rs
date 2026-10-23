//! Boards in the UI: the small bar floating over a board's selection (colour, shape, text size,
//! connector style; boards hide the Design panel until asked for), and the swatches it and the
//! board tools' flyouts share.

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use figcraft_doc::{BOARD_COLORS, BoardItem, BoardShape, NodeKind, Routing, Stamp, StrokeCap};
use figcraft_engine::board::{BoardTool, TEXT_SIZES};
use serde_json::{Value, json};

use crate::FigApp;
use crate::a11y;
use crate::i18n::tr;
use crate::icons::{self, Icon};
use crate::theme::{Tokens, semibold};
use crate::toolbar::Options;

/// What a swatch shows.
pub(crate) enum Mark {
    Color(figcraft_color::Color),
    Shape(BoardShape),
    Stamp(Stamp),
}

/// One choice of a board tool's flyout: its name, the tool's params, whether it is the tool's now.
pub(crate) struct Choice {
    pub label: &'static str,
    pub params: Value,
    pub on: bool,
    pub mark: Mark,
}

/// The choices a board tool's flyout offers.
pub(crate) fn choices(options: Options, board: &BoardTool) -> Vec<Choice> {
    match options {
        Options::Colors => BOARD_COLORS
            .iter()
            .map(|(n, c)| Choice { label: color_label(n), params: json!({"color": n}), on: board.sticky == *c, mark: Mark::Color(*c) })
            .collect(),
        Options::Shapes => BoardShape::ALL
            .iter()
            .map(|b| Choice { label: b.label(), params: json!({"shape": b.id()}), on: board.shape == *b, mark: Mark::Shape(*b) })
            .collect(),
        Options::Stamps => Stamp::ALL
            .iter()
            .map(|b| Choice { label: b.label(), params: json!({"stamp": b.id()}), on: board.stamp == *b, mark: Mark::Stamp(*b) })
            .collect(),
        Options::None => Vec::new(),
    }
}

/// A board colour's name as the UI says it.
fn color_label(name: &str) -> &'static str {
    match name {
        "yellow" => "Yellow",
        "orange" => "Orange",
        "red" => "Red",
        "pink" => "Pink",
        "violet" => "Violet",
        "blue" => "Blue",
        "green" => "Green",
        _ => "Gray",
    }
}

/// Paint `mark` in `r`: a colour chip, a shape's outline in `ink`, or a stamp badge.
pub(crate) fn paint_choice(p: &egui::Painter, r: Rect, mark: &Mark, ink: Color32) {
    match mark {
        Mark::Color(c) => {
            p.rect_filled(r, CornerRadius::same(4), c32(*c));
            p.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0, Color32::from_black_alpha(40)), StrokeKind::Inside);
        }
        Mark::Shape(s) => {
            let path = s.path(14.0, 14.0);
            let path = figcraft_geom::Affine::translate((1.0, 1.0)) * path;
            icons::paint_outlines(p, r, 16.0, &[(&path, false)], ink, 1.2);
        }
        Mark::Stamp(s) => {
            p.circle_filled(r.center(), r.width().min(r.height()) / 2.0, c32(s.color()));
            let parts: Vec<(figcraft_geom::BezPath, bool)> =
                s.mark().iter().filter_map(|m| figcraft_geom::BezPath::from_svg(m.d).ok().map(|b| (b, m.filled))).collect();
            let refs: Vec<(&figcraft_geom::BezPath, bool)> = parts.iter().map(|(b, f)| (b, *f)).collect();
            icons::paint_outlines(p, r, figcraft_doc::STAMP_SIZE, &refs, Color32::WHITE, 3.0);
        }
    }
}

fn c32(c: figcraft_color::Color) -> Color32 {
    let [r, g, b, a] = c.to_rgba8();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// What the selection has that the bar can change.
#[derive(Default)]
struct Has {
    color: bool,
    shape: Option<BoardShape>,
    text: bool,
    connector: Option<(Routing, bool)>,
}

fn has(app: &FigApp) -> Option<Has> {
    let ds = app.session.ds().ok()?;
    let mut h = Has::default();
    for id in &ds.selection {
        let n = ds.doc.node(*id)?;
        match n.board.as_ref()? {
            BoardItem::Sticky { .. } | BoardItem::Ink => h.color = true,
            BoardItem::Shape { shape } => {
                h.color = true;
                h.text = true;
                h.shape = Some(*shape);
            }
            BoardItem::Connector(c) => {
                h.color = true;
                h.connector = Some((c.routing, n.stroke.cap_end != StrokeCap::None));
            }
            BoardItem::Label { .. } => h.text = true,
            BoardItem::Stamp { .. } => {}
        }
    }
    (h.color || h.text || h.connector.is_some()).then_some(h)
}

const CELL: f32 = 28.0;
const SWATCH: f32 = 20.0;
const GAP: f32 = 4.0;

/// The bar above a board's selection, while nothing is being dragged.
pub fn show(app: &mut FigApp, ctx: &egui::Context) {
    if app.session.tool.busy() || !app.session.doc().is_ok_and(figcraft_engine::board::is_board) {
        return;
    }
    let Some(h) = has(app) else { return };
    let Some(b) = app.session.ds().ok().and_then(|ds| ds.doc.bounds_of(&ds.selection)) else { return };
    let (top, bottom) = (app.to_screen(b.x0, b.y0), app.to_screen(b.x1, b.y1));
    let mut w = 8.0;
    if h.color {
        w += BOARD_COLORS.len() as f32 * (SWATCH + GAP) + 8.0;
    }
    if h.shape.is_some() {
        w += CELL + 12.0 + 8.0;
    }
    if h.text {
        w += TEXT_SIZES.len() as f32 * (CELL + 2.0) + 8.0;
    }
    if h.connector.is_some() {
        w += 3.0 * (CELL + 2.0) + 8.0;
    }
    let size = vec2(w, 40.0);
    let canvas = app.canvas.rect;
    let cx = ((top.x + bottom.x) / 2.0).clamp(canvas.min.x + w / 2.0 + 8.0, (canvas.max.x - w / 2.0 - 8.0).max(canvas.min.x + w / 2.0 + 8.0));
    let above = top.y - 14.0 - size.y;
    let y = if above > canvas.min.y + 8.0 { above } else { (bottom.y + 14.0).min(canvas.max.y - size.y - 80.0) };
    let at = pos2(cx - w / 2.0, y);
    let t = Tokens::get(ctx);
    egui::Area::new(egui::Id::new("board_bar")).fixed_pos(at).order(egui::Order::Middle).show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(size, Sense::click());
        a11y::container_bounds(ui, rect);
        let p = ui.painter();
        p.rect_filled(rect.expand(2.0).translate(vec2(0.0, 1.0)), CornerRadius::same(12), Color32::from_black_alpha(18));
        p.rect_filled(rect, CornerRadius::same(10), t.panel);
        p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, t.border), StrokeKind::Inside);
        let mut x = rect.min.x + 8.0;
        let cy = rect.center().y;
        if h.color {
            for (i, (name, c)) in BOARD_COLORS.iter().enumerate() {
                let r = Rect::from_center_size(pos2(x + SWATCH / 2.0, cy), vec2(SWATCH, SWATCH));
                let resp = ui.interact(r, ui.id().with(("bb_color", i)), Sense::click()).on_hover_text(tr(color_label(name)));
                a11y::button(&resp, tr(color_label(name)));
                if resp.hovered() {
                    ui.painter().rect_filled(r.expand(2.0), CornerRadius::same(6), t.hover);
                }
                paint_choice(ui.painter(), r, &Mark::Color(*c), t.icon);
                if resp.clicked() {
                    app.run("board.setColor", json!({"color": name}));
                }
                x += SWATCH + GAP;
            }
            x = divider(ui, &t, x, cy);
        }
        if let Some(shape) = h.shape {
            let r = Rect::from_min_size(pos2(x, cy - CELL / 2.0), vec2(CELL + 12.0, CELL));
            let resp = ui.interact(r, ui.id().with("bb_shape"), Sense::click()).on_hover_text(tr("Shape"));
            a11y::menu_button(&resp, tr("Shape"), a11y::menu_open(&resp));
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(5), t.hover);
            }
            paint_choice(ui.painter(), Rect::from_center_size(pos2(r.min.x + CELL / 2.0, cy), vec2(18.0, 18.0)), &Mark::Shape(shape), t.icon);
            icons::paint(ui.painter(), Rect::from_center_size(pos2(r.max.x - 7.0, cy), vec2(12.0, 12.0)), Icon::Chevron, t.icon_secondary);
            crate::right::menu_popup(&resp).show(|ui| {
                for b in BoardShape::ALL {
                    let art = |p: &egui::Painter, row: Rect, ink: Color32| {
                        paint_choice(p, Rect::from_center_size(pos2(row.max.x - 16.0, row.center().y), vec2(16.0, 16.0)), &Mark::Shape(b), ink)
                    };
                    if crate::right::menu_item_with(ui, tr(b.label()), "", 24.0, art) {
                        app.run("board.setStyle", json!({"shape": b.id()}));
                        ui.close();
                    }
                }
            });
            x = divider(ui, &t, x + CELL + 12.0 + GAP, cy);
        }
        if h.text {
            for (label, size) in TEXT_SIZES {
                let r = Rect::from_center_size(pos2(x + CELL / 2.0, cy), vec2(CELL, CELL));
                let resp = ui.interact(r, ui.id().with(("bb_size", label)), Sense::click()).on_hover_text(tr("Text size"));
                a11y::button(&resp, &format!("{} {label}", tr("Text size")));
                if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(5), t.hover);
                }
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, semibold(11.0), t.text);
                if resp.clicked() {
                    app.run("board.setStyle", json!({"fontSize": size}));
                }
                x += CELL + 2.0;
            }
            x = divider(ui, &t, x, cy);
        }
        if let Some((routing, arrow)) = h.connector {
            let cells = [
                (Icon::Line, tr("Straight"), routing == Routing::Straight, json!({"style": "straight"})),
                (Icon::Connector, tr("Elbow"), routing == Routing::Elbow, json!({"style": "elbow"})),
                (Icon::Arrow, tr("Arrow at the end"), arrow, json!({"endCap": if arrow { "none" } else { "triangleArrow" }})),
            ];
            for (i, (icon, tip, on, params)) in cells.into_iter().enumerate() {
                let r = Rect::from_center_size(pos2(x + CELL / 2.0, cy), vec2(CELL, CELL));
                if crate::toolbar::tool_cell(ui, &t, r, ("bb_line", i), icon, on, tip).clicked() {
                    app.run("board.setStyle", params);
                }
                x += CELL + 2.0;
            }
        }
    });
}

/// A thin separator after `x`; where the next part starts.
fn divider(ui: &Ui, t: &Tokens, x: f32, cy: f32) -> f32 {
    ui.painter().line_segment([pos2(x + 2.0, cy - 10.0), pos2(x + 2.0, cy + 10.0)], Stroke::new(1.0, t.border));
    x + 8.0
}

/// The name of the person who placed the stamp under the pointer, beside it.
pub fn stamp_author(app: &FigApp, painter: &egui::Painter, pointer: Option<Pos2>) {
    let (Some(at), Ok(ds)) = (pointer, app.session.ds()) else { return };
    let Some(n) = ds.hover.and_then(|h| ds.doc.node(h)) else { return };
    let Some(BoardItem::Stamp { author, .. }) = &n.board else { return };
    if author.is_empty() || !matches!(n.kind, NodeKind::Frame) {
        return;
    }
    let t = Tokens::get(painter.ctx());
    let galley = painter.layout_no_wrap(author.clone(), semibold(11.0), t.text);
    let r = Rect::from_min_size(at + vec2(14.0, 14.0), galley.size() + vec2(12.0, 6.0));
    painter.rect_filled(r, CornerRadius::same(4), t.menu);
    painter.galley(r.min + vec2(6.0, 3.0), galley, t.menu_text);
}
