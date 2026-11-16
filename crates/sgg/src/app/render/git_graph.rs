//! Commit-graph gutter painter (Zed `git_graph` inspired).
//!
//! Each history row owns a small canvas that paints the slice of lanes / edges
//! for that row index, plus the commit node.

use gpui_kit::*;
use sgg_core::{is_uncommitted_oid, CommitLine, CommitLineSegment, CurveKind, HistoryGraph, GRAPH_ACCENT_COUNT};

const LANE_WIDTH: f32 = 14.0;
const LEFT_PADDING: f32 = 8.0;
const LINE_WIDTH: f32 = 2.0;
const CIRCLE_RADIUS: f32 = 4.0;
const CIRCLE_STROKE: f32 = 1.5;
const CIRCLE_HALO: f32 = 2.0;
/// Vertical size of one history row (graph gutter + commit text).
pub const ROW_HEIGHT: f32 = 64.0;

pub fn accent_color(idx: usize) -> Hsla {
    const COLORS: [Hsla; GRAPH_ACCENT_COUNT] = [
        hsla(210. / 360., 0.70, 0.55, 1.),
        hsla(145. / 360., 0.55, 0.45, 1.),
        hsla(25. / 360., 0.75, 0.55, 1.),
        hsla(280. / 360., 0.45, 0.55, 1.),
        hsla(340. / 360., 0.65, 0.55, 1.),
        hsla(175. / 360., 0.55, 0.45, 1.),
        hsla(45. / 360., 0.70, 0.50, 1.),
        hsla(0. / 360., 0.60, 0.55, 1.),
    ];
    COLORS[idx % GRAPH_ACCENT_COUNT]
}

pub fn graph_width(max_lanes: usize) -> Pixels {
    let lanes = max_lanes.max(1).min(12);
    px(LEFT_PADDING * 2.0 + LANE_WIDTH * lanes as f32)
}

fn lane_center_x(origin_x: Pixels, lane: usize) -> Pixels {
    origin_x + px(LEFT_PADDING) + px(LANE_WIDTH) * lane as f32 + px(LANE_WIDTH / 2.0)
}

fn fill_circle(cx: Pixels, cy: Pixels, r: Pixels, color: Hsla, window: &mut Window) {
    let mut builder = PathBuilder::fill();
    builder.move_to(point(cx + r, cy));
    builder.arc_to(point(r, r), px(0.), false, true, point(cx - r, cy));
    builder.arc_to(point(r, r), px(0.), false, true, point(cx + r, cy));
    builder.close();
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn draw_commit_circle(
    cx: Pixels,
    cy: Pixels,
    color: Hsla,
    background: Hsla,
    window: &mut Window,
) {
    fill_circle(cx, cy, px(CIRCLE_RADIUS + CIRCLE_HALO), background, window);
    fill_circle(cx, cy, px(CIRCLE_RADIUS), color, window);
    fill_circle(
        cx,
        cy,
        px((CIRCLE_RADIUS - CIRCLE_STROKE).max(0.)),
        background,
        window,
    );
}

fn stroke_line(from: Point<Pixels>, to: Point<Pixels>, color: Hsla, window: &mut Window) {
    if (from.x - to.x).abs() < px(0.1) && (from.y - to.y).abs() < px(0.1) {
        return;
    }
    let mut builder = PathBuilder::stroke(px(LINE_WIDTH));
    builder.move_to(from);
    builder.line_to(to);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn stroke_curve(
    from: Point<Pixels>,
    to: Point<Pixels>,
    curve_kind: CurveKind,
    color: Hsla,
    window: &mut Window,
) {
    let mut builder = PathBuilder::stroke(px(LINE_WIDTH));
    builder.move_to(from);
    match curve_kind {
        CurveKind::Merge => {
            // Drop then bend into the child column (Zed merge).
            let ctrl_a = point(from.x, to.y);
            let ctrl_b = point((from.x + to.x) / 2.0, to.y);
            builder.cubic_bezier_to(to, ctrl_a, ctrl_b);
        }
        CurveKind::Checkout => {
            // Descend then slide into the parent column (Zed checkout).
            let ctrl_a = point(from.x, (from.y + to.y) / 2.0);
            let ctrl_b = point(to.x, (from.y + to.y) / 2.0);
            builder.cubic_bezier_to(to, ctrl_a, ctrl_b);
        }
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn paint_vertical_slice(
    x: Pixels,
    row: usize,
    line_start: usize,
    line_end: usize,
    y0: Pixels,
    y1: Pixels,
    y_mid: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    let r = px(CIRCLE_RADIUS);
    // Bleed 1px past the row edges so adjacent per-row canvases seal without gaps.
    let bleed = px(1.);
    let (from_y, to_y) = if row == line_start && row == line_end {
        // Parent is the next painted commit on this same visual row spacing —
        // still draw a stub below the node so the lane reads as connected.
        (y_mid + r, y1 + bleed)
    } else if row == line_start {
        (y_mid + r, y1 + bleed)
    } else if row == line_end {
        (y0 - bleed, y_mid - r)
    } else {
        (y0 - bleed, y1 + bleed)
    };
    stroke_line(point(x, from_y), point(x, to_y), color, window);
}

fn paint_line_at_row(
    graph: &HistoryGraph,
    line: &CommitLine,
    row: usize,
    ox: Pixels,
    y0: Pixels,
    y1: Pixels,
    y_mid: Pixels,
    muted: Hsla,
    window: &mut Window,
) {
    let start = line.full_interval.start;
    let end = line.full_interval.end;
    if row < start || row > end {
        return;
    }
    // Lines that start at the WIP tip stay gray for the whole segment.
    let color = if graph
        .commits
        .get(start)
        .is_some_and(|c| is_uncommitted_oid(&c.oid))
    {
        muted
    } else {
        accent_color(line.color_idx)
    };
    let mut col = line.child_column;
    let mut cursor_row = start;

    for segment in &line.segments {
        match segment {
            CommitLineSegment::Straight { to_row } => {
                let seg_end = *to_row;
                if row >= cursor_row && row <= seg_end {
                    let x = lane_center_x(ox, col);
                    // Treat commit endpoints relative to the whole line.
                    let local_start = cursor_row;
                    let is_line_start = row == start && row == local_start;
                    let is_line_end = row == end && row == seg_end;
                    if is_line_start || is_line_end || (row > start && row < end) {
                        paint_vertical_slice(x, row, start, end, y0, y1, y_mid, color, window);
                    } else if row > local_start && row < seg_end {
                        let bleed = px(1.);
                        stroke_line(
                            point(x, y0 - bleed),
                            point(x, y1 + bleed),
                            color,
                            window,
                        );
                    } else if row == local_start {
                        stroke_line(point(x, y_mid), point(x, y1 + px(1.)), color, window);
                    } else if row == seg_end {
                        stroke_line(point(x, y0 - px(1.)), point(x, y_mid), color, window);
                    }
                }
                cursor_row = seg_end;
            }
            CommitLineSegment::Curve {
                to_column,
                on_row,
                curve_kind,
            } => {
                if row == *on_row {
                    let from_x = lane_center_x(ox, col);
                    let to_x = lane_center_x(ox, *to_column);
                    // Merge curves sit just below the child; checkout above parent.
                    let (from, to) = match curve_kind {
                        CurveKind::Merge => (
                            point(from_x, y_mid + px(CIRCLE_RADIUS)),
                            point(to_x, y1),
                        ),
                        CurveKind::Checkout => (
                            point(from_x, y0),
                            point(to_x, y_mid - px(CIRCLE_RADIUS)),
                        ),
                    };
                    stroke_curve(from, to, *curve_kind, color, window);
                } else if row > cursor_row && row < *on_row {
                    let x = lane_center_x(ox, col);
                    paint_vertical_slice(x, row, start, end, y0, y1, y_mid, color, window);
                } else if row > *on_row && row <= end {
                    let x = lane_center_x(ox, *to_column);
                    paint_vertical_slice(x, row, start, end, y0, y1, y_mid, color, window);
                }
                col = *to_column;
                cursor_row = *on_row;
            }
        }
    }
}

/// Paint one history-row slice of the graph.
pub fn paint_row(
    graph: &HistoryGraph,
    row: usize,
    bounds: Bounds<Pixels>,
    background: Hsla,
    muted: Hsla,
    window: &mut Window,
) {
    let Some(entry) = graph.commits.get(row) else {
        return;
    };
    let y0 = bounds.origin.y;
    let y1 = y0 + bounds.size.height;
    let y_mid = y0 + bounds.size.height / 2.0;
    let ox = bounds.origin.x;

    for line in &graph.lines {
        paint_line_at_row(graph, line, row, ox, y0, y1, y_mid, muted, window);
    }

    let node = if is_uncommitted_oid(&entry.oid) {
        muted
    } else {
        accent_color(entry.color_idx)
    };
    draw_commit_circle(
        lane_center_x(ox, entry.lane),
        y_mid,
        node,
        background,
        window,
    );
}

/// Canvas element for a single history row graph gutter.
///
/// Uses `h_full` so the painted lane fills the commit row (avoids gaps when the
/// row is taller than a fixed canvas height due to padding).
pub fn row_canvas(
    graph: HistoryGraph,
    row: usize,
    background: Hsla,
    muted: Hsla,
) -> impl IntoElement {
    let width = graph_width(graph.max_lanes);
    canvas(
        move |_bounds, _, _| (),
        move |bounds, (), window, _cx| {
            paint_row(&graph, row, bounds, background, muted, window);
        },
    )
    .w(width)
    .h_full()
    .min_h(px(ROW_HEIGHT))
    .flex_shrink_0()
}
