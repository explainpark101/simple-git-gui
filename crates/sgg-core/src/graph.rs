//! Commit-graph lane layout inspired by Zed `git_ui::git_graph::GraphData`.
//!
//! Newest commits first (same order as `git log`). First parent continues the
//! current lane; additional parents open merge lanes that curve into the child.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveKind {
    Merge,
    Checkout,
}

#[derive(Debug, Clone)]
pub enum CommitLineSegment {
    Straight {
        to_row: usize,
    },
    Curve {
        to_column: usize,
        on_row: usize,
        curve_kind: CurveKind,
    },
}

#[derive(Debug, Clone)]
pub struct CommitLine {
    pub child_column: usize,
    pub full_interval: std::ops::Range<usize>,
    pub color_idx: usize,
    pub segments: Vec<CommitLineSegment>,
}

#[derive(Debug, Clone)]
pub struct GraphCommit {
    pub oid: String,
    pub lane: usize,
    pub color_idx: usize,
}

#[derive(Debug, Clone, Default)]
pub struct HistoryGraph {
    pub commits: Vec<GraphCommit>,
    pub lines: Vec<CommitLine>,
    pub max_lanes: usize,
}

/// Input row for lane assignment (newest → oldest).
#[derive(Debug, Clone)]
pub struct GraphCommitInput {
    pub oid: String,
    pub parents: Vec<String>,
}

#[derive(Clone)]
enum LaneState {
    Empty,
    Active {
        /// Kept for parity with Zed lane state (useful when debugging layout).
        #[allow(dead_code)]
        parent: String,
        #[allow(dead_code)]
        child: String,
        color: Option<usize>,
        starting_row: usize,
        starting_col: usize,
        destination_column: Option<usize>,
        segments: Vec<CommitLineSegment>,
    },
}

impl LaneState {
    fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    fn to_commit_lines(
        &mut self,
        ending_row: usize,
        lane_column: usize,
        parent_column: usize,
        parent_color: usize,
    ) -> Option<CommitLine> {
        let state = std::mem::replace(self, LaneState::Empty);
        match state {
            LaneState::Active {
                color,
                starting_row,
                starting_col,
                destination_column,
                mut segments,
                ..
            } => {
                let final_destination = destination_column.unwrap_or(parent_column);
                let final_color = color.unwrap_or(parent_color);
                finalize_segments(
                    &mut segments,
                    starting_row,
                    ending_row,
                    lane_column,
                    final_destination,
                );
                Some(CommitLine {
                    child_column: starting_col,
                    full_interval: starting_row..ending_row,
                    color_idx: final_color,
                    segments,
                })
            }
            LaneState::Empty => None,
        }
    }
}

fn finalize_segments(
    segments: &mut Vec<CommitLineSegment>,
    starting_row: usize,
    ending_row: usize,
    lane_column: usize,
    final_destination: usize,
) {
    match segments.last_mut() {
        Some(CommitLineSegment::Straight { to_row }) if *to_row == usize::MAX => {
            if final_destination != lane_column {
                *to_row = ending_row.saturating_sub(1);
                let curved = CommitLineSegment::Curve {
                    to_column: final_destination,
                    on_row: ending_row,
                    curve_kind: CurveKind::Checkout,
                };
                if *to_row == starting_row {
                    let last = segments.len() - 1;
                    segments[last] = curved;
                } else {
                    segments.push(curved);
                }
            } else {
                *to_row = ending_row;
            }
        }
        Some(CommitLineSegment::Curve {
            on_row,
            to_column,
            curve_kind,
        }) if *on_row == usize::MAX => {
            if *to_column == usize::MAX {
                *to_column = final_destination;
            }
            if matches!(curve_kind, CurveKind::Merge) {
                *on_row = starting_row + 1;
                if *on_row < ending_row {
                    if *to_column != final_destination {
                        segments.push(CommitLineSegment::Straight {
                            to_row: ending_row.saturating_sub(1),
                        });
                        segments.push(CommitLineSegment::Curve {
                            to_column: final_destination,
                            on_row: ending_row,
                            curve_kind: CurveKind::Checkout,
                        });
                    } else {
                        segments.push(CommitLineSegment::Straight { to_row: ending_row });
                    }
                } else if *to_column != final_destination {
                    segments.push(CommitLineSegment::Curve {
                        to_column: final_destination,
                        on_row: ending_row,
                        curve_kind: CurveKind::Checkout,
                    });
                }
            } else {
                *on_row = ending_row;
                if *to_column != final_destination {
                    segments.push(CommitLineSegment::Straight { to_row: ending_row });
                    segments.push(CommitLineSegment::Curve {
                        to_column: final_destination,
                        on_row: ending_row,
                        curve_kind: CurveKind::Checkout,
                    });
                }
            }
        }
        Some(CommitLineSegment::Curve {
            on_row, to_column, ..
        }) => {
            if *on_row < ending_row {
                if *to_column != final_destination {
                    segments.push(CommitLineSegment::Straight {
                        to_row: ending_row.saturating_sub(1),
                    });
                    segments.push(CommitLineSegment::Curve {
                        to_column: final_destination,
                        on_row: ending_row,
                        curve_kind: CurveKind::Checkout,
                    });
                } else {
                    segments.push(CommitLineSegment::Straight { to_row: ending_row });
                }
            } else if *to_column != final_destination {
                segments.push(CommitLineSegment::Curve {
                    to_column: final_destination,
                    on_row: ending_row,
                    curve_kind: CurveKind::Checkout,
                });
            }
        }
        _ => {}
    }
}

/// Default accent cycle length (matches Zed-style multi-branch coloring).
pub const GRAPH_ACCENT_COUNT: usize = 8;

/// Synthetic oid for the working-tree / index WIP row at the tip of History.
pub const UNCOMMITTED_OID: &str = "__uncommitted__";

pub fn is_uncommitted_oid(oid: &str) -> bool {
    oid == UNCOMMITTED_OID
}

/// Build a lane graph from newest-first commits with parent OIDs.
/// `accent_count` is the number of rotating branch colors (Zed accent cycle).
pub fn build_history_graph(commits: &[GraphCommitInput], accent_count: usize) -> HistoryGraph {
    let accent_count = accent_count.max(1);
    let mut lane_states: Vec<LaneState> = Vec::new();
    let mut lane_colors: HashMap<usize, usize> = HashMap::new();
    let mut parent_to_lanes: HashMap<String, Vec<usize>> = HashMap::new();
    let mut next_color: usize = 0;
    let mut out_commits: Vec<GraphCommit> = Vec::with_capacity(commits.len());
    let mut lines: Vec<CommitLine> = Vec::new();
    let mut max_lanes = 0usize;

    let first_empty = |lane_states: &mut Vec<LaneState>| -> usize {
        if let Some(idx) = lane_states.iter().position(LaneState::is_empty) {
            idx
        } else {
            lane_states.push(LaneState::Empty);
            lane_states.len() - 1
        }
    };

    let get_lane_color = |lane_colors: &mut HashMap<usize, usize>,
                          next_color: &mut usize,
                          lane_idx: usize|
     -> usize {
        *lane_colors.entry(lane_idx).or_insert_with(|| {
            let c = *next_color;
            *next_color = (*next_color + 1) % accent_count;
            c
        })
    };

    for commit in commits {
        let commit_row = out_commits.len();
        let commit_lane = parent_to_lanes
            .get(&commit.oid)
            .and_then(|lanes| lanes.iter().min().copied())
            .unwrap_or_else(|| first_empty(&mut lane_states));

        let commit_color = get_lane_color(&mut lane_colors, &mut next_color, commit_lane);

        if let Some(lanes) = parent_to_lanes.remove(&commit.oid) {
            for lane_column in lanes {
                let state = &mut lane_states[lane_column];
                if let LaneState::Active {
                    starting_row,
                    segments,
                    ..
                } = state
                {
                    if let Some(CommitLineSegment::Curve {
                        to_column,
                        curve_kind: CurveKind::Merge,
                        ..
                    }) = segments.first_mut()
                    {
                        let curve_row = *starting_row + 1;
                        let would_overlap =
                            if lane_column != commit_lane && curve_row < commit_row {
                                out_commits[curve_row..commit_row]
                                    .iter()
                                    .any(|c| c.lane == commit_lane)
                            } else {
                                false
                            };
                        if would_overlap {
                            *to_column = lane_column;
                        }
                    }
                }
                if let Some(line) =
                    state.to_commit_lines(commit_row, lane_column, commit_lane, commit_color)
                {
                    lines.push(line);
                }
            }
        }

        for (parent_idx, parent) in commit.parents.iter().enumerate() {
            if parent_idx == 0 {
                if commit_lane >= lane_states.len() {
                    lane_states.resize(commit_lane + 1, LaneState::Empty);
                }
                lane_states[commit_lane] = LaneState::Active {
                    parent: parent.clone(),
                    child: commit.oid.clone(),
                    color: Some(commit_color),
                    starting_col: commit_lane,
                    starting_row: commit_row,
                    destination_column: None,
                    segments: vec![CommitLineSegment::Straight { to_row: usize::MAX }],
                };
                parent_to_lanes
                    .entry(parent.clone())
                    .or_default()
                    .push(commit_lane);
            } else {
                let new_lane = first_empty(&mut lane_states);
                lane_states[new_lane] = LaneState::Active {
                    parent: parent.clone(),
                    child: commit.oid.clone(),
                    color: None,
                    starting_col: commit_lane,
                    starting_row: commit_row,
                    destination_column: None,
                    segments: vec![CommitLineSegment::Curve {
                        to_column: usize::MAX,
                        on_row: usize::MAX,
                        curve_kind: CurveKind::Merge,
                    }],
                };
                parent_to_lanes
                    .entry(parent.clone())
                    .or_default()
                    .push(new_lane);
            }
        }

        max_lanes = max_lanes.max(lane_states.len());
        out_commits.push(GraphCommit {
            oid: commit.oid.clone(),
            lane: commit_lane,
            color_idx: commit_color,
        });
    }

    // Close any still-active lanes at EOF (dangling edges into unloaded history).
    let end_row = out_commits.len();
    for lane_column in 0..lane_states.len() {
        let state = &mut lane_states[lane_column];
        if state.is_empty() {
            continue;
        }
        let color = match state {
            LaneState::Active { color, .. } => color.unwrap_or(0),
            LaneState::Empty => 0,
        };
        if let Some(line) = state.to_commit_lines(end_row, lane_column, lane_column, color) {
            lines.push(line);
        }
    }

    HistoryGraph {
        commits: out_commits,
        lines,
        max_lanes: max_lanes.max(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_history_stays_on_lane_zero() {
        let commits = vec![
            GraphCommitInput {
                oid: "c0".into(),
                parents: vec!["c1".into()],
            },
            GraphCommitInput {
                oid: "c1".into(),
                parents: vec!["c2".into()],
            },
            GraphCommitInput {
                oid: "c2".into(),
                parents: vec![],
            },
        ];
        let g = build_history_graph(&commits, 6);
        assert_eq!(g.commits.len(), 3);
        assert!(g.commits.iter().all(|c| c.lane == 0));
    }

    #[test]
    fn merge_opens_second_lane() {
        // Newest first: merge commit with two parents, then each parent chain.
        let commits = vec![
            GraphCommitInput {
                oid: "merge".into(),
                parents: vec!["main".into(), "feat".into()],
            },
            GraphCommitInput {
                oid: "feat".into(),
                parents: vec!["base".into()],
            },
            GraphCommitInput {
                oid: "main".into(),
                parents: vec!["base".into()],
            },
            GraphCommitInput {
                oid: "base".into(),
                parents: vec![],
            },
        ];
        let g = build_history_graph(&commits, 6);
        assert_eq!(g.commits[0].lane, 0);
        assert!(g.max_lanes >= 2);
        assert!(!g.lines.is_empty());
    }
}
