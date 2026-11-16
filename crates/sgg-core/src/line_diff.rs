//! Per-line working-tree diff marks for editor gutter decorations (VS Code style).

use std::path::Path;

use crate::shell::{run_git, GitCliError};

/// Kind of change relative to `HEAD` for a line (or deletion boundary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineChangeKind {
    /// Present in the working tree / new file, absent in `HEAD`.
    Added,
    /// Present on both sides but content differs.
    Modified,
    /// Removed from `HEAD`; shown as a boundary mark in the gutter.
    Deleted,
}

/// A gutter-oriented change against `HEAD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineChange {
    /// Half-open 0-based line range in the current (new) file.
    Lines {
        kind: LineChangeKind,
        start: usize,
        end: usize,
    },
    /// Deletion boundary before 0-based line `before` (or at EOF when `before == line_count`).
    DeletedBefore {
        before: usize,
    },
}

/// Line-level changes for `rel_path` vs `HEAD` (`git diff -U0 HEAD -- path`).
///
/// Untracked files are treated as fully added.
pub fn list_file_line_changes(
    repo: &Path,
    rel_path: &str,
) -> Result<Vec<LineChange>, GitCliError> {
    let diff = match run_git(
        repo,
        &["diff", "-U0", "--no-color", "HEAD", "--", rel_path],
    ) {
        Ok(text) => text,
        // Empty repo / missing HEAD — treat as untracked content if the file exists.
        Err(_) => String::new(),
    };

    if !diff.is_empty() {
        return Ok(parse_unified_diff_hunks(&diff));
    }

    if is_tracked_path(repo, rel_path)? {
        return Ok(Vec::new());
    }

    let full = repo.join(rel_path);
    let Ok(text) = std::fs::read_to_string(&full) else {
        return Ok(Vec::new());
    };
    let lines = count_lines(&text);
    if lines == 0 {
        return Ok(Vec::new());
    }
    Ok(vec![LineChange::Lines {
        kind: LineChangeKind::Added,
        start: 0,
        end: lines,
    }])
}

fn is_tracked_path(repo: &Path, rel_path: &str) -> Result<bool, GitCliError> {
    match run_git(repo, &["ls-files", "--error-unmatch", "--", rel_path]) {
        Ok(_) => Ok(true),
        Err(GitCliError::Failed { .. }) => Ok(false),
        Err(err) => Err(err),
    }
}

fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let mut n = text.bytes().filter(|&b| b == b'\n').count();
    if !text.ends_with('\n') {
        n += 1;
    }
    n
}

/// Map 0-based line index → UTF-8 byte start offset in `text`.
/// `line == line_count` returns `text.len()` (EOF).
pub fn line_start_offset(text: &str, line: usize) -> usize {
    if line == 0 {
        return 0;
    }
    let mut seen = 0usize;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            seen += 1;
            if seen == line {
                return i + 1;
            }
        }
    }
    text.len()
}

/// UTF-8 byte range covering 0-based lines `[start, end)`.
pub fn lines_byte_range(text: &str, start: usize, end: usize) -> std::ops::Range<usize> {
    let start = start.min(end);
    line_start_offset(text, start)..line_start_offset(text, end)
}

/// Parse `@@ -old,count +new,count @@` hunks from a unified diff (`-U0` friendly).
pub fn parse_unified_diff_hunks(diff: &str) -> Vec<LineChange> {
    let mut out = Vec::new();
    for line in diff.lines() {
        let Some(rest) = line.strip_prefix("@@ ") else {
            continue;
        };
        let Some((meta, _)) = rest.split_once(" @@") else {
            continue;
        };
        let Some((old_part, new_part)) = meta.split_once(' ') else {
            continue;
        };
        let Some((_old_start, old_count)) = parse_hunk_side(old_part, '-') else {
            continue;
        };
        let Some((new_start, new_count)) = parse_hunk_side(new_part, '+') else {
            continue;
        };

        if old_count == 0 && new_count > 0 {
            let start = new_start.saturating_sub(1);
            out.push(LineChange::Lines {
                kind: LineChangeKind::Added,
                start,
                end: start + new_count,
            });
        } else if new_count == 0 && old_count > 0 {
            // `+N,0` → deletions after line N of the new file (boundary before 0-based N).
            out.push(LineChange::DeletedBefore { before: new_start });
        } else if new_count > 0 {
            let start = new_start.saturating_sub(1);
            out.push(LineChange::Lines {
                kind: LineChangeKind::Modified,
                start,
                end: start + new_count,
            });
        }
    }
    out
}

fn parse_hunk_side(part: &str, sigil: char) -> Option<(usize, usize)> {
    let rest = part.strip_prefix(sigil)?;
    let (start_s, count_s) = match rest.split_once(',') {
        Some((s, c)) => (s, Some(c)),
        None => (rest, None),
    };
    let start: usize = start_s.parse().ok()?;
    let count = match count_s {
        Some(c) => c.parse().ok()?,
        None => {
            // Omitted count defaults to 1, except a pure `0` start means empty.
            if start == 0 {
                0
            } else {
                1
            }
        }
    };
    Some((start, count))
}

/// Kind of a display line inside a unified-diff buffer (`+` / `-` / context).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnifiedDiffLineKind {
    Added,
    Deleted,
    Context,
    Meta,
}

/// UTF-8 ranges for each non-context line in a unified diff (for gutter / fill).
pub fn unified_diff_line_marks(text: &str) -> Vec<(UnifiedDiffLineKind, std::ops::Range<usize>)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let len = line.len();
        let body = line.strip_suffix('\n').unwrap_or(line);
        let kind = if body.starts_with("+++ ")
            || body.starts_with("--- ")
            || body.starts_with("diff ")
            || body.starts_with("index ")
            || body.starts_with("@@")
            || body.starts_with("new file")
            || body.starts_with("deleted file")
            || body.starts_with("similarity ")
            || body.starts_with("rename ")
        {
            UnifiedDiffLineKind::Meta
        } else if body.starts_with('+') {
            UnifiedDiffLineKind::Added
        } else if body.starts_with('-') {
            UnifiedDiffLineKind::Deleted
        } else {
            UnifiedDiffLineKind::Context
        };
        if matches!(
            kind,
            UnifiedDiffLineKind::Added | UnifiedDiffLineKind::Deleted
        ) {
            out.push((kind, offset..offset + len));
        }
        offset += len;
    }
    out
}

/// Per-side line marks for a split (old | new) diff view, from unified hunks.
#[derive(Debug, Clone, Default)]
pub struct SplitDiffMarks {
    /// 0-based half-open ranges on the old file (deleted / modified).
    pub old: Vec<(LineChangeKind, usize, usize)>,
    /// 0-based half-open ranges on the new file (added / modified).
    pub new: Vec<(LineChangeKind, usize, usize)>,
}

/// One display row in an aligned old|new split (empty counterpart cells included).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignedSplitLine {
    /// 1-based source line number, or `None` for a blank padding row.
    pub source_line: Option<u32>,
    /// Line text without trailing newline (empty when padding).
    pub text: String,
    /// Change kind for non-context source rows; `None` for context / padding.
    pub kind: Option<LineChangeKind>,
}

/// Side-by-side buffers with empty rows opposite pure adds/deletes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlignedSplitView {
    pub old: Vec<AlignedSplitLine>,
    pub new: Vec<AlignedSplitLine>,
}

impl AlignedSplitView {
    /// Plain source text for one side (no line-number prefix — use a gutter UI).
    pub fn plain_side(&self, old_side: bool) -> String {
        let lines = if old_side { &self.old } else { &self.new };
        let mut out = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&line.text);
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out
    }

    /// 1-based source line per display row (`None` = blank padding).
    pub fn source_lines(&self, old_side: bool) -> &[AlignedSplitLine] {
        if old_side {
            &self.old
        } else {
            &self.new
        }
    }

    /// Digit width for a custom line-number gutter.
    pub fn line_number_digits(&self) -> usize {
        let max = self
            .old
            .iter()
            .chain(self.new.iter())
            .filter_map(|l| l.source_line)
            .max()
            .unwrap_or(1);
        max.to_string().len().max(1)
    }

    /// Display-row indices that are shared context (both sides present, unchanged).
    ///
    /// Used to keep scroll sync locked to non-diff content when panes diverge.
    pub fn context_display_rows(&self) -> Vec<usize> {
        self.old
            .iter()
            .zip(self.new.iter())
            .enumerate()
            .filter(|(_, (o, n))| {
                o.source_line.is_some()
                    && n.source_line.is_some()
                    && o.kind.is_none()
                    && n.kind.is_none()
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Map a display row to the nearest context row at or above it (else below).
    pub fn nearest_context_row(&self, display_row: usize) -> Option<usize> {
        let rows = self.context_display_rows();
        if rows.is_empty() {
            return None;
        }
        match rows.binary_search(&display_row) {
            Ok(i) => Some(rows[i]),
            Err(i) => {
                if i > 0 {
                    Some(rows[i - 1])
                } else {
                    Some(rows[0])
                }
            }
        }
    }

    /// 0-based display-line change marks for gutter fills (old side).
    pub fn old_marks(&self) -> Vec<(LineChangeKind, usize, usize)> {
        coalesce_marks(
            self.old
                .iter()
                .enumerate()
                .filter_map(|(i, l)| l.kind.map(|k| (k, i))),
        )
    }

    /// 0-based display-line change marks for gutter fills (new side).
    pub fn new_marks(&self) -> Vec<(LineChangeKind, usize, usize)> {
        coalesce_marks(
            self.new
                .iter()
                .enumerate()
                .filter_map(|(i, l)| l.kind.map(|k| (k, i))),
        )
    }
}

fn coalesce_marks(
    marks: impl Iterator<Item = (LineChangeKind, usize)>,
) -> Vec<(LineChangeKind, usize, usize)> {
    let mut out: Vec<(LineChangeKind, usize, usize)> = Vec::new();
    for (kind, ix) in marks {
        if let Some(last) = out.last_mut() {
            if last.0 == kind && last.2 == ix {
                last.2 = ix + 1;
                continue;
            }
        }
        out.push((kind, ix, ix + 1));
    }
    out
}

/// Build an aligned split from a unified / Git patch (ideally `-U999999`).
///
/// Change runs pair delete/add on the same display row (empty cell when one
/// side is missing). Returns `None` when the patch has no hunk bodies.
pub fn align_split_from_unified(patch: &str) -> Option<AlignedSplitView> {
    let mut view = AlignedSplitView::default();
    let mut in_hunk = false;
    let mut old_ln: u32 = 0;
    let mut new_ln: u32 = 0;
    let mut saw_body = false;
    let mut pending_del: Vec<(u32, String)> = Vec::new();
    let mut pending_add: Vec<(u32, String)> = Vec::new();

    let flush = |view: &mut AlignedSplitView,
                 dels: &mut Vec<(u32, String)>,
                 adds: &mut Vec<(u32, String)>| {
        let n = dels.len().max(adds.len());
        for i in 0..n {
            let old = if i < dels.len() {
                let (ln, text) = dels[i].clone();
                AlignedSplitLine {
                    source_line: Some(ln),
                    text,
                    kind: Some(if i < adds.len() {
                        LineChangeKind::Modified
                    } else {
                        LineChangeKind::Deleted
                    }),
                }
            } else {
                AlignedSplitLine {
                    source_line: None,
                    text: String::new(),
                    kind: None,
                }
            };
            let new = if i < adds.len() {
                let (ln, text) = adds[i].clone();
                AlignedSplitLine {
                    source_line: Some(ln),
                    text,
                    kind: Some(if i < dels.len() {
                        LineChangeKind::Modified
                    } else {
                        LineChangeKind::Added
                    }),
                }
            } else {
                AlignedSplitLine {
                    source_line: None,
                    text: String::new(),
                    kind: None,
                }
            };
            view.old.push(old);
            view.new.push(new);
        }
        dels.clear();
        adds.clear();
    };

    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("@@ ") {
            flush(&mut view, &mut pending_del, &mut pending_add);
            let meta = rest.split_once(" @@").map(|(m, _)| m).unwrap_or(rest);
            let Some((old_part, new_part)) = meta.split_once(' ') else {
                in_hunk = false;
                continue;
            };
            let Some((old_start, _)) = parse_hunk_side(old_part, '-') else {
                in_hunk = false;
                continue;
            };
            let Some((new_start, _)) = parse_hunk_side(new_part, '+') else {
                in_hunk = false;
                continue;
            };
            old_ln = old_start as u32;
            new_ln = new_start as u32;
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        if line.starts_with('\\') {
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let (sigil, content) = line.split_at(1);
        match sigil {
            " " => {
                flush(&mut view, &mut pending_del, &mut pending_add);
                saw_body = true;
                view.old.push(AlignedSplitLine {
                    source_line: Some(old_ln),
                    text: content.to_string(),
                    kind: None,
                });
                view.new.push(AlignedSplitLine {
                    source_line: Some(new_ln),
                    text: content.to_string(),
                    kind: None,
                });
                old_ln = old_ln.saturating_add(1);
                new_ln = new_ln.saturating_add(1);
            }
            "-" => {
                saw_body = true;
                pending_del.push((old_ln, content.to_string()));
                old_ln = old_ln.saturating_add(1);
            }
            "+" => {
                saw_body = true;
                pending_add.push((new_ln, content.to_string()));
                new_ln = new_ln.saturating_add(1);
            }
            _ => {
                flush(&mut view, &mut pending_del, &mut pending_add);
                in_hunk = false;
            }
        }
    }
    flush(&mut view, &mut pending_del, &mut pending_add);

    saw_body.then_some(view)
}

/// Build split-view marks from a unified diff (same hunk headers as `-U0` or normal).
pub fn split_diff_marks_from_unified(diff: &str) -> SplitDiffMarks {
    let mut marks = SplitDiffMarks::default();
    for line in diff.lines() {
        let Some(rest) = line.strip_prefix("@@ ") else {
            continue;
        };
        let Some((meta, _)) = rest.split_once(" @@") else {
            continue;
        };
        let Some((old_part, new_part)) = meta.split_once(' ') else {
            continue;
        };
        let Some((old_start, old_count)) = parse_hunk_side(old_part, '-') else {
            continue;
        };
        let Some((new_start, new_count)) = parse_hunk_side(new_part, '+') else {
            continue;
        };

        if old_count > 0 {
            let start = old_start.saturating_sub(1);
            let kind = if new_count == 0 {
                LineChangeKind::Deleted
            } else {
                LineChangeKind::Modified
            };
            marks.old.push((kind, start, start + old_count));
        }
        if new_count > 0 {
            let start = new_start.saturating_sub(1);
            let kind = if old_count == 0 {
                LineChangeKind::Added
            } else {
                LineChangeKind::Modified
            };
            marks.new.push((kind, start, start + new_count));
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_add_modify_delete_hunks() {
        let diff = "\
diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -10,0 +11,2 @@
+one
+two
@@ -20,2 +22,2 @@
-old
-old2
+new
+new2
@@ -30,3 +31,0 @@
-gone
-gone2
-gone3
";
        let marks = parse_unified_diff_hunks(diff);
        assert_eq!(
            marks,
            vec![
                LineChange::Lines {
                    kind: LineChangeKind::Added,
                    start: 10,
                    end: 12,
                },
                LineChange::Lines {
                    kind: LineChangeKind::Modified,
                    start: 21,
                    end: 23,
                },
                LineChange::DeletedBefore { before: 31 },
            ]
        );
    }

    #[test]
    fn line_byte_ranges() {
        let text = "a\nbb\nccc\n";
        assert_eq!(lines_byte_range(text, 0, 1), 0..2);
        assert_eq!(lines_byte_range(text, 1, 3), 2..9);
        assert_eq!(line_start_offset(text, 3), 9);
    }

    #[test]
    fn unified_display_marks() {
        let text = "@@ -1 +1 @@\n-old\n+new\n context\n";
        let marks = unified_diff_line_marks(text);
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].0, UnifiedDiffLineKind::Deleted);
        assert_eq!(marks[1].0, UnifiedDiffLineKind::Added);
    }

    #[test]
    fn aligned_split_pads_add_and_delete() {
        let patch = "\
--- a/f
+++ b/f
@@ -1,3 +1,3 @@
 keep
-old
+new
 only
";
        let view = align_split_from_unified(patch).expect("hunk body");
        // keep | keep, old|new (paired), only|only
        assert_eq!(view.old.len(), 3);
        assert_eq!(view.new.len(), 3);
        assert_eq!(view.old[0].text, "keep");
        assert_eq!(view.old[1].text, "old");
        assert_eq!(view.new[1].text, "new");
        assert_eq!(view.old[1].kind, Some(LineChangeKind::Modified));
        assert_eq!(view.new[1].kind, Some(LineChangeKind::Modified));
        assert_eq!(view.old[2].text, "only");
        let rendered = view.plain_side(true);
        assert!(rendered.contains("keep\n"));
        assert!(rendered.contains("old\n"));
        assert!(!rendered.contains('│'));
        assert_eq!(view.nearest_context_row(1), Some(0));
    }

    #[test]
    fn aligned_split_empty_cell_for_pure_add() {
        let patch = "@@ -1,1 +1,2 @@\n keep\n+extra\n";
        let view = align_split_from_unified(patch).expect("hunk body");
        assert_eq!(view.old.len(), 2);
        assert!(view.old[1].source_line.is_none());
        assert_eq!(view.new[1].text, "extra");
        assert_eq!(view.new[1].kind, Some(LineChangeKind::Added));
    }
}
