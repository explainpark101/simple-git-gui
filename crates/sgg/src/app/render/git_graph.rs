//! Commit-graph gutter painter (Zed `git_graph` inspired).
//!
//! Each history row owns a small canvas that paints the slice of lanes / edges
//! for that row index, plus the commit node. Lane hit-targets sit above the
//! canvas so hover can show branch names for that color.
//!
//! Merge / checkout fillets follow Zed: leave the commit circle sideways,
//! run a short horizontal, then a quadratic corner into the destination lane
//! so lines never end in mid-air.

use std::collections::HashMap;

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sgg_core::{
    is_uncommitted_oid, CommitLine, CommitLineSegment, CurveKind, HistoryGraph, GRAPH_ACCENT_COUNT,
};

const LANE_WIDTH: f32 = 14.0;
const LEFT_PADDING: f32 = 8.0;
const LINE_WIDTH: f32 = 1.75;
const CIRCLE_RADIUS: f32 = 3.5;
const CIRCLE_STROKE: f32 = 1.35;
const CIRCLE_HALO: f32 = 1.5;
/// Vertical size of one history row (Zed-like dense table row).
pub const ROW_HEIGHT: f32 = 28.0;

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

/// Zed merge fillet: leave the child circle sideways, horizontal, then bend down
/// into `to_x`, ending at `end_y` on the destination lane.
fn stroke_merge_fillet(
    from_x: Pixels,
    from_y: Pixels,
    to_x: Pixels,
    end_y: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    let going_right = to_x > from_x;
    let shift = px(CIRCLE_RADIUS + CIRCLE_STROKE);
    let signed_shift = if going_right { shift } else { -shift };
    let merge_start = point(from_x + signed_shift, from_y);

    let available_w = (to_x - merge_start.x).abs();
    let available_h = (end_y - merge_start.y).abs().max(px(1.));
    let curve_w = px(LANE_WIDTH / 3.0).min(available_w);
    let curve_h = (available_h / 3.0).min(available_h);
    let signed_w = if going_right { curve_w } else { -curve_w };

    let curve_start = point(to_x - signed_w, merge_start.y);
    let curve_end = point(to_x, merge_start.y + curve_h);
    let curve_control = point(to_x, merge_start.y);

    let mut builder = PathBuilder::stroke(px(LINE_WIDTH));
    builder.move_to(merge_start);
    builder.line_to(curve_start);
    builder.curve_to(curve_end, curve_control);
    builder.line_to(point(to_x, end_y));
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Zed checkout fillet: descend on `from_x`, bend into the parent row, then
/// slide horizontally to the parent circle edge at `to_y`.
fn stroke_checkout_fillet(
    from_x: Pixels,
    from_y: Pixels,
    to_x: Pixels,
    to_y: Pixels,
    color: Hsla,
    window: &mut Window,
) {
    let going_right = to_x > from_x;
    let shift = px(CIRCLE_RADIUS + CIRCLE_STROKE);
    let target_x = if going_right { to_x - shift } else { to_x + shift };

    let available_w = (target_x - from_x).abs();
    let available_h = (to_y - from_y).abs().max(px(1.));
    let curve_w = px(LANE_WIDTH / 3.0).min(available_w);
    let curve_h = (available_h / 3.0).min(available_h);
    let signed_w = if going_right { curve_w } else { -curve_w };

    let curve_start = point(from_x, to_y - curve_h);
    let curve_end = point(from_x + signed_w, to_y);
    let curve_control = point(from_x, to_y);

    let mut builder = PathBuilder::stroke(px(LINE_WIDTH));
    builder.move_to(point(from_x, from_y));
    builder.line_to(curve_start);
    builder.curve_to(curve_end, curve_control);
    builder.line_to(point(target_x, to_y));
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
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
    let r = px(CIRCLE_RADIUS);
    let bleed = px(1.);

    for (seg_idx, segment) in line.segments.iter().enumerate() {
        let is_last = seg_idx + 1 == line.segments.len();
        match segment {
            CommitLineSegment::Straight { to_row } => {
                let seg_end = *to_row;
                if row >= cursor_row && row <= seg_end {
                    // Merge elbow on the start row already exits at y1 on the
                    // destination column — skip the overlapping straight stub.
                    let merge_sealed = seg_idx
                        .checked_sub(1)
                        .and_then(|i| line.segments.get(i))
                        .is_some_and(|s| {
                            matches!(
                                s,
                                CommitLineSegment::Curve {
                                    curve_kind: CurveKind::Merge,
                                    ..
                                }
                            )
                        })
                        && row == start
                        && row == cursor_row;
                    if !merge_sealed {
                        let x = lane_center_x(ox, col);
                        let next_is_checkout = line.segments.get(seg_idx + 1).is_some_and(|s| {
                            matches!(
                                s,
                                CommitLineSegment::Curve {
                                    curve_kind: CurveKind::Checkout,
                                    ..
                                }
                            )
                        });
                        let continues_below = row < seg_end || (next_is_checkout && row < end);
                        let at_line_start = row == start;
                        let at_true_end = is_last && row == end && row == seg_end;

                        let from_y = if at_line_start {
                            y_mid + r
                        } else {
                            y0 - bleed
                        };
                        let to_y = if at_true_end {
                            y_mid - r
                        } else if continues_below || (row == seg_end && !is_last) {
                            y1 + bleed
                        } else {
                            y_mid
                        };
                        stroke_line(point(x, from_y), point(x, to_y), color, window);
                    }
                }
                cursor_row = seg_end;
            }
            CommitLineSegment::Curve {
                to_column,
                on_row,
                curve_kind,
            } => {
                match curve_kind {
                    CurveKind::Merge => {
                        // Elbow lives on the merge (child) row — connects into
                        // the circle edge, then fillets down into the side lane.
                        if row == start {
                            let from_x = lane_center_x(ox, col);
                            let to_x = lane_center_x(ox, *to_column);
                            let end_y = if *on_row == start {
                                y_mid + r
                            } else {
                                y1 + bleed
                            };
                            stroke_merge_fillet(from_x, y_mid, to_x, end_y, color, window);
                        }
                        // Rows between start and on_row / end: vertical on dest.
                        if row > start && row < *on_row {
                            let x = lane_center_x(ox, *to_column);
                            stroke_line(
                                point(x, y0 - bleed),
                                point(x, y1 + bleed),
                                color,
                                window,
                            );
                        }
                        // At on_row (usually tip or first spacer): seal into node
                        // or continue downward for a following Straight.
                        if row == *on_row && *on_row > start {
                            let x = lane_center_x(ox, *to_column);
                            if *on_row == end {
                                stroke_line(
                                    point(x, y0 - bleed),
                                    point(x, y_mid - r),
                                    color,
                                    window,
                                );
                            } else {
                                stroke_line(
                                    point(x, y0 - bleed),
                                    point(x, y1 + bleed),
                                    color,
                                    window,
                                );
                            }
                        } else if row > *on_row && row <= end {
                            let x = lane_center_x(ox, *to_column);
                            if row == end {
                                stroke_line(
                                    point(x, y0 - bleed),
                                    point(x, y_mid - r),
                                    color,
                                    window,
                                );
                            } else {
                                stroke_line(
                                    point(x, y0 - bleed),
                                    point(x, y1 + bleed),
                                    color,
                                    window,
                                );
                            }
                        }
                    }
                    CurveKind::Checkout => {
                        if row == *on_row {
                            let from_x = lane_center_x(ox, col);
                            let to_x = lane_center_x(ox, *to_column);
                            stroke_checkout_fillet(from_x, y0, to_x, y_mid, color, window);
                        } else if row >= cursor_row && row < *on_row {
                            let x = lane_center_x(ox, col);
                            let from_y = if row == start { y_mid + r } else { y0 - bleed };
                            stroke_line(point(x, from_y), point(x, y1 + bleed), color, window);
                        }
                    }
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

/// Column occupied by `line` at `row`, if the line covers that row.
fn column_at_row(line: &CommitLine, row: usize) -> Option<usize> {
    let start = line.full_interval.start;
    let end = line.full_interval.end;
    if row < start || row > end {
        return None;
    }
    let mut col = line.child_column;
    let mut cursor_row = start;
    for segment in &line.segments {
        match segment {
            CommitLineSegment::Straight { to_row } => {
                if row >= cursor_row && row <= *to_row {
                    return Some(col);
                }
                cursor_row = *to_row;
            }
            CommitLineSegment::Curve {
                to_column, on_row, ..
            } => {
                if row < *on_row {
                    if row >= cursor_row {
                        return Some(col);
                    }
                } else if row == *on_row {
                    return Some(*to_column);
                } else if row <= end {
                    return Some(*to_column);
                }
                col = *to_column;
                cursor_row = *on_row;
            }
        }
    }
    Some(col)
}

/// Graph color active on `lane` at `row`, if any.
pub fn color_at_lane(graph: &HistoryGraph, row: usize, lane: usize) -> Option<usize> {
    if let Some(entry) = graph.commits.get(row) {
        if entry.lane == lane {
            if is_uncommitted_oid(&entry.oid) {
                return None;
            }
            return Some(entry.color_idx);
        }
    }
    for line in &graph.lines {
        if let Some(col) = column_at_row(line, row) {
            if col == lane {
                if graph
                    .commits
                    .get(line.full_interval.start)
                    .is_some_and(|c| is_uncommitted_oid(&c.oid))
                {
                    continue;
                }
                return Some(line.color_idx);
            }
        }
    }
    None
}

fn branch_tooltip(
    names: Vec<String>,
    color_idx: usize,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |window, cx| {
        let names = names.clone();
        let color = accent_color(color_idx);
        Tooltip::element(move |_, cx| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(names.iter().map(|name| {
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .size(px(8.))
                                .rounded_full()
                                .flex_shrink_0()
                                .bg(color),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().popover_foreground)
                                .child(name.clone()),
                        )
                }))
        })
        .build(window, cx)
    }
}

/// Canvas + lane hover targets for a single history row graph gutter.
pub fn row_canvas(
    graph: HistoryGraph,
    row: usize,
    background: Hsla,
    muted: Hsla,
    color_branches: HashMap<usize, Vec<String>>,
) -> impl IntoElement {
    let width = graph_width(graph.max_lanes);
    let lane_count = graph.max_lanes.max(1).min(12);
    let paint_graph = graph.clone();

    div()
        .relative()
        .w(width)
        .h_full()
        .min_h(px(ROW_HEIGHT))
        .flex_shrink_0()
        .child(
            canvas(
                move |_bounds, _, _| (),
                move |bounds, (), window, _cx| {
                    paint_row(&paint_graph, row, bounds, background, muted, window);
                },
            )
            .absolute()
            .inset_0()
            .w_full()
            .h_full(),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .pl(px(LEFT_PADDING))
                .pr(px(LEFT_PADDING))
                .children((0..lane_count).map(|lane| {
                    let color_idx = color_at_lane(&graph, row, lane);
                    let names = color_idx
                        .and_then(|idx| color_branches.get(&idx))
                        .cloned()
                        .unwrap_or_default();
                    let tip = (!names.is_empty()).then(|| {
                        let idx = color_idx.unwrap_or(0);
                        branch_tooltip(names, idx)
                    });
                    div()
                        .id(SharedString::from(format!("hist-graph-lane:{row}:{lane}")))
                        .w(px(LANE_WIDTH))
                        .h_full()
                        .flex_shrink_0()
                        .when_some(tip, |this, build| this.tooltip(build))
                })),
        )
}
