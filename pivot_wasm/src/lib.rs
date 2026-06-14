use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, console};

use crate::log_utility::log_to_console;

mod log_utility;

const FIXED_COL: &str = "competition";
const GROUP_COL: &str = "season";
const WINNER_COL: &str = "winner";
const RUNNER_UP_COL: &str = "runner_up";

// ─── Data Structures ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DataRow {
    pub fields: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
struct Group {
    competition: String,
    row_indices: Vec<usize>,
}

/// Describes a column group (winner or runner_up)
#[derive(Clone, Debug, Serialize)]
struct ColGroup {
    key: String,   // "winner" or "runner_up"
    label: String, // display label
    expanded: bool,
    members: Vec<String>, // unique team names (sorted)
}

// ─── Core Engine ────────────────────────────────────────────────────────────

#[wasm_bindgen]
#[derive(Serialize)]
pub struct PivotEngine {
    data: Vec<DataRow>,
    /// All raw columns found in data (for width storage)
    columns: Vec<String>,
    col_widths: Vec<f64>,
    groups: Vec<Group>,
    expanded_rows: HashSet<usize>,
    col_groups: Vec<ColGroup>,
    scroll_y: f64,
    scroll_x: f64,
    viewport_height: f64,
    viewport_width: f64,
    canvas_width: f64,
    canvas_height: f64,
}

// ─── Visible column descriptor ───────────────────────────────────────────────

/// A single visible column slot.
#[derive(Clone, Debug)]
struct VisCol {
    /// logical key used to look up cell value in DataRow.fields
    key: String,
    /// display label (may differ from key for member cols)
    label: String,
    /// index into self.col_widths
    width_idx: usize,
    /// which col_group index this belongs to (None = fixed/season)
    col_group_idx: Option<usize>,
}

#[wasm_bindgen]
impl PivotEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(data: JsValue) -> Result<PivotEngine, JsValue> {
        let rows: Vec<DataRow> = serde_wasm_bindgen::from_value(data)
            .map_err(|e| JsValue::from_str(&format!("Invalid data: {}", e)))?;

        let mut engine = PivotEngine {
            data: rows,
            columns: Vec::new(),
            col_widths: Vec::new(),
            groups: Vec::new(),
            expanded_rows: HashSet::new(),
            col_groups: Vec::new(),
            scroll_y: 0.0,
            scroll_x: 0.0,
            viewport_height: 0.0,
            viewport_width: 0.0,
            canvas_width: 0.0,
            canvas_height: 0.0,
        };
        engine.rebuild_groups();
        Ok(engine)
    }

    pub fn set_viewport(&mut self, width: f64, height: f64) {
        self.canvas_width = width;
        self.canvas_height = height;
        self.viewport_height = height - 54.0 - 10.0;
        self.viewport_width = width - 10.0;
    }

    pub fn total_content_height(&self) -> f64 {
        let row_height = 25.0;
        let mut total = self.groups.len() as f64 * row_height;
        for (gi, group) in self.groups.iter().enumerate() {
            if self.expanded_rows.contains(&gi) {
                total += group.row_indices.len() as f64 * row_height;
            }
        }
        total
    }

    pub fn set_scroll_x(&mut self, scroll_x: f64) {
        let total_w: f64 = self
            .visible_cols()
            .iter()
            .map(|vc| self.col_widths[vc.width_idx])
            .sum();
        let max_scroll = (total_w - self.viewport_width).max(0.0);
        self.scroll_x = scroll_x.max(0.0).min(max_scroll);
    }

    pub fn get_scroll_x(&self) -> f64 {
        self.scroll_x
    }

    pub fn hscrollbar_thumb_rect(&self) -> Vec<f64> {
        let total_w: f64 = self
            .visible_cols()
            .iter()
            .map(|vc| self.col_widths[vc.width_idx])
            .sum();
        let track_w = self.viewport_width;
        if total_w <= track_w {
            return vec![0.0, 0.0, 0.0, 0.0];
        }
        let thumb_w = (track_w / total_w * track_w).max(30.0);
        let thumb_x = (self.scroll_x / total_w) * track_w;
        vec![thumb_x, self.canvas_height - 9.0, thumb_w, 8.0]
    }

    pub fn scrollbar_thumb_rect(&self) -> Vec<f64> {
        let track_h = self.viewport_height;
        let content_h = self.total_content_height();
        if content_h <= track_h {
            return vec![0.0, 0.0, 0.0, 0.0];
        }
        let thumb_h = (track_h / content_h * track_h).max(30.0);
        let thumb_y = (self.scroll_y / content_h) * track_h;
        vec![self.canvas_width - 10.0, 54.0 + thumb_y, 8.0, thumb_h]
    }

    pub fn set_scroll_y(&mut self, scroll_y: f64) {
        let max_scroll = self.total_content_height() - self.viewport_height;
        self.scroll_y = scroll_y.max(0.0).min(max_scroll.max(0.0));
    }

    pub fn get_scroll_y(&self) -> f64 {
        self.scroll_y
    }

    pub fn toggle_row(&mut self, row_idx: usize) {
        if self.expanded_rows.contains(&row_idx) {
            self.expanded_rows.remove(&row_idx);
        } else {
            self.expanded_rows.insert(row_idx);
        }
    }

    pub fn hit_test_row(
        &self,
        py: f64,
        scroll_y: f64,
        start_y: f64,
        header_h: f64,
        row_height: f64,
    ) -> i32 {
        let relative_y = py - (start_y + header_h) + scroll_y;
        if relative_y < 0.0 {
            return -1;
        }
        let mut y = 0.0;
        for (gi, group) in self.groups.iter().enumerate() {
            if relative_y >= y && relative_y < y + row_height {
                return gi as i32;
            }
            y += row_height;
            if self.expanded_rows.contains(&gi) {
                let children_h = group.row_indices.len() as f64 * row_height;
                if relative_y >= y && relative_y < y + children_h {
                    return -1;
                }
                y += children_h;
            }
        }
        -1
    }

    pub fn update_data(&mut self, data: JsValue) -> Result<(), JsValue> {
        self.data = serde_wasm_bindgen::from_value(data)
            .map_err(|e| JsValue::from_str(&format!("Invalid data: {}", e)))?;
        self.columns.clear();
        self.col_widths.clear();
        self.expanded_rows.clear();
        self.col_groups.clear();
        self.rebuild_groups();
        Ok(())
    }

    /// Toggle a column group by index (0 = winner, 1 = runner_up)
    pub fn toggle_col_group(&mut self, group_idx: usize) {
        if let Some(cg) = self.col_groups.get_mut(group_idx) {
            cg.expanded = !cg.expanded;
        }
    }

    pub fn is_col_group_expanded(&self, group_idx: usize) -> bool {
        self.col_groups
            .get(group_idx)
            .map(|cg| cg.expanded)
            .unwrap_or(false)
    }

    pub fn col_group_count(&self) -> usize {
        self.col_groups.len()
    }

    pub fn set_col_width(&mut self, col_idx: usize, width: f64) {
        if col_idx < self.col_widths.len() {
            self.col_widths[col_idx] = width.max(30.0);
        }
    }

    pub fn get_col_widths(&self) -> Vec<f64> {
        self.col_widths.clone()
    }

    pub fn get_visible_col_widths(&self) -> Vec<f64> {
        self.visible_cols()
            .iter()
            .map(|vc| self.col_widths[vc.width_idx])
            .collect()
    }

    pub fn set_visible_col_width(&mut self, visible_idx: usize, width: f64) {
        let vcs = self.visible_cols();
        if let Some(vc) = vcs.get(visible_idx) {
            self.col_widths[vc.width_idx] = width.max(30.0);
        }
    }

    pub fn swap_visible_columns(&mut self, from_visible_idx: usize, to_visible_idx: usize) {
        if from_visible_idx == to_visible_idx {
            return;
        }
        if from_visible_idx == 0 || to_visible_idx == 0 {
            return;
        }

        let vcs = self.visible_cols();
        if from_visible_idx >= vcs.len() || to_visible_idx >= vcs.len() {
            return;
        }

        let from_vc = vcs[from_visible_idx].clone();
        let to_vc = vcs[to_visible_idx].clone();

        match (from_vc.col_group_idx, to_vc.col_group_idx) {
            // ── Same col-group, both are member cols ("group::member" key) ───
            (Some(fgi), Some(tgi))
                if fgi == tgi && from_vc.key.contains("::") && to_vc.key.contains("::") =>
            {
                let bare = |k: &str| k.splitn(2, "::").nth(1).unwrap_or(k).to_string();
                let fk_bare = bare(&from_vc.key);
                let tk_bare = bare(&to_vc.key);
                let members = &mut self.col_groups[fgi].members;
                if let (Some(fi), Some(ti)) = (
                    members.iter().position(|m| m == &fk_bare),
                    members.iter().position(|m| m == &tk_bare),
                ) {
                    members.swap(fi, ti);
                    // Swap widths so each team keeps its pixel width after reorder
                    let fwi = self.width_idx_for(&from_vc.key);
                    let twi = self.width_idx_for(&to_vc.key);
                    self.col_widths.swap(fwi, twi);
                }
            }

            // ── Different col-groups: swap the whole groups ──────────────────
            (Some(fgi), Some(tgi)) if fgi != tgi => {
                self.col_groups.swap(fgi, tgi);
                // Width slots are keyed by string — order comes from col_groups vec,
                // no col_widths swap needed here.
            }

            // season col or any other unhandled case — ignore
            _ => {}
        }
    }

    pub fn get_col_name_at_visible_idx(&self, visible_idx: usize) -> String {
        self.visible_cols()
            .get(visible_idx)
            .map(|vc| vc.label.clone())
            .unwrap_or_default()
    }

    /// hit-test for col-group toggle button in header row 1.
    /// Returns col_group index or -1.
    pub fn hit_test_col_group_toggle(&self, px: f64, py: f64) -> i32 {
        if py < 0.0 || py > 30.0 {
            return -1;
        }
        let fixed_w = self.fixed_col_width();
        let mut x = fixed_w - self.scroll_x;

        // season col — not a group, skip
        let season_wi = self.width_idx_for(GROUP_COL);
        let season_w = self.col_widths[season_wi];
        x += season_w;

        for (cgi, cg) in self.col_groups.iter().enumerate() {
            let header_w = self.col_group_header_width(cgi);
            let btn_x = x + 4.0;
            let btn_end = btn_x + 16.0;
            // only test if in visible area
            if btn_x < self.viewport_width && btn_end > fixed_w {
                if px >= btn_x && px <= btn_end {
                    return cgi as i32;
                }
            }
            x += header_w;
        }
        -1
    }

    pub fn render(&mut self, ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
        if self.data.is_empty() {
            self.draw_empty_message(ctx)?;
            return Ok(());
        }
        if self.columns.is_empty() || self.col_widths.is_empty() {
            self.compute_columns_and_widths(ctx)?;
        }

        let row_height = 25.0;
        let header_h1 = 30.0;
        let header_h2 = 24.0;
        let total_header_h = header_h1 + header_h2;
        let start_y = 0.0;
        let scroll_bar_w = 10.0;
        let scroll_bar_h = 10.0;

        let width = self.canvas_width;
        let height = self.canvas_height;
        let viewport_y = start_y + total_header_h;
        let viewport_h = self.viewport_height;
        let viewport_w = self.viewport_width;

        let fixed_w = self.fixed_col_width();

        // ── Clear ────────────────────────────────────────────────────────
        ctx.set_fill_style(&JsValue::from_str("#ffffff"));
        ctx.fill_rect(0.0, 0.0, width, height);

        // Build screen positions for all visible cols
        let col_positions = self.build_col_screen_positions(fixed_w);

        // ════════════════════════════════════════════════════════════════
        // PASS 1 — scrollable data cells
        // ════════════════════════════════════════════════════════════════
        ctx.save();
        ctx.begin_path();
        ctx.rect(fixed_w, viewport_y, viewport_w - fixed_w, viewport_h);
        ctx.clip();

        let mut y_cursor = 0.0;
        for (gi, group) in self.groups.iter().enumerate() {
            let is_expanded = self.expanded_rows.contains(&gi);
            let row_top = y_cursor;
            let row_bottom = row_top + row_height;
            let group_visible =
                row_bottom >= self.scroll_y && row_top <= self.scroll_y + viewport_h;

            if group_visible {
                let screen_y = viewport_y + (row_top - self.scroll_y);
                let fill_color = if gi % 2 == 0 { "#ffffff" } else { "#fafafa" };
                ctx.set_fill_style(&JsValue::from_str(fill_color));
                ctx.fill_rect(fixed_w, screen_y, viewport_w - fixed_w, row_height);

                for (vc, sx, cw) in &col_positions {
                    if vc.key == FIXED_COL {
                        continue;
                    }
                    if sx + cw <= fixed_w || *sx >= viewport_w {
                        continue;
                    }
                    ctx.set_stroke_style(&JsValue::from_str("#cccccc"));
                    ctx.set_line_width(1.0);
                    ctx.stroke_rect(*sx, screen_y, *cw, row_height);
                }
            }
            y_cursor += row_height;

            if is_expanded {
                for &data_idx in &group.row_indices {
                    let row = &self.data[data_idx];
                    let child_top = y_cursor;
                    let child_bottom = child_top + row_height;
                    let child_visible =
                        child_bottom >= self.scroll_y && child_top <= self.scroll_y + viewport_h;

                    if child_visible {
                        let screen_y = viewport_y + (child_top - self.scroll_y);
                        ctx.set_fill_style(&JsValue::from_str("#eff6ff"));
                        ctx.fill_rect(fixed_w, screen_y, viewport_w - fixed_w, row_height);

                        for (vc, sx, cw) in &col_positions {
                            if vc.key == FIXED_COL {
                                continue;
                            }
                            if sx + cw <= fixed_w || *sx >= viewport_w {
                                continue;
                            }

                            ctx.set_stroke_style(&JsValue::from_str("#bfdbfe"));
                            ctx.set_line_width(1.0);
                            ctx.stroke_rect(*sx, screen_y, *cw, row_height);

                            let cell_text = self.get_cell_value(row, vc);
                            if !cell_text.is_empty() {
                                ctx.set_fill_style(&JsValue::from_str("#1e40af"));
                                ctx.set_font("12px sans-serif");
                                ctx.set_text_align("center");
                                ctx.set_text_baseline("alphabetic");
                                ctx.fill_text(&cell_text, sx + cw / 2.0, screen_y + 18.0)
                                    .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
                            }
                        }
                    }
                    y_cursor += row_height;
                    if y_cursor - row_height > self.scroll_y + viewport_h {
                        break;
                    }
                }
            }
            if y_cursor > self.scroll_y + viewport_h {
                break;
            }
        }
        ctx.restore();

        // ════════════════════════════════════════════════════════════════
        // PASS 2 — pinned competition column
        // ════════════════════════════════════════════════════════════════
        ctx.save();
        ctx.begin_path();
        ctx.rect(0.0, viewport_y, fixed_w, viewport_h);
        ctx.clip();

        let mut y_cursor = 0.0;
        for (gi, group) in self.groups.iter().enumerate() {
            let is_expanded = self.expanded_rows.contains(&gi);
            let row_top = y_cursor;
            let row_bottom = row_top + row_height;
            let group_visible =
                row_bottom >= self.scroll_y && row_top <= self.scroll_y + viewport_h;

            if group_visible {
                let screen_y = viewport_y + (row_top - self.scroll_y);
                let fill_color = if gi % 2 == 0 { "#ffffff" } else { "#fafafa" };
                ctx.set_fill_style(&JsValue::from_str(fill_color));
                ctx.fill_rect(0.0, screen_y, fixed_w, row_height);
                ctx.set_stroke_style(&JsValue::from_str("#cccccc"));
                ctx.set_line_width(1.0);
                ctx.stroke_rect(0.0, screen_y, fixed_w, row_height);

                let btn_size = 16.0;
                let btn_x = 4.0;
                let btn_y = screen_y + (row_height - btn_size) / 2.0;
                ctx.set_fill_style(&JsValue::from_str(if is_expanded {
                    "#dbeafe"
                } else {
                    "#e2e8f0"
                }));
                ctx.begin_path();
                ctx.round_rect_with_f64(btn_x, btn_y, btn_size, btn_size, 3.0)?;
                ctx.fill();
                ctx.set_stroke_style(&JsValue::from_str(if is_expanded {
                    "#3b82f6"
                } else {
                    "#94a3b8"
                }));
                ctx.set_line_width(1.0);
                ctx.stroke();
                ctx.set_fill_style(&JsValue::from_str(if is_expanded {
                    "#1d4ed8"
                } else {
                    "#475569"
                }));
                ctx.set_font("bold 11px sans-serif");
                ctx.set_text_align("center");
                ctx.set_text_baseline("middle");
                let arrow = if is_expanded { "▼" } else { "▶" };
                ctx.fill_text(arrow, btn_x + btn_size / 2.0, btn_y + btn_size / 2.0)
                    .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

                ctx.set_fill_style(&JsValue::from_str("#000000"));
                ctx.set_font("bold 12px sans-serif");
                let btn_end = btn_x + btn_size + 4.0;
                let remaining = fixed_w - btn_end;
                ctx.set_text_align("center");
                ctx.set_text_baseline("alphabetic");
                ctx.fill_text(
                    &group.competition,
                    btn_end + remaining / 2.0,
                    screen_y + 18.0,
                )
                .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
            }
            y_cursor += row_height;

            if is_expanded {
                for &data_idx in &group.row_indices {
                    let row = &self.data[data_idx];
                    let child_top = y_cursor;
                    let child_bottom = child_top + row_height;
                    let child_visible =
                        child_bottom >= self.scroll_y && child_top <= self.scroll_y + viewport_h;

                    if child_visible {
                        let screen_y = viewport_y + (child_top - self.scroll_y);
                        ctx.set_fill_style(&JsValue::from_str("#eff6ff"));
                        ctx.fill_rect(0.0, screen_y, fixed_w, row_height);
                        ctx.set_stroke_style(&JsValue::from_str("#bfdbfe"));
                        ctx.set_line_width(1.0);
                        ctx.stroke_rect(0.0, screen_y, fixed_w, row_height);
                        let season_text =
                            row.fields.get(GROUP_COL).map(|s| s.as_str()).unwrap_or("");
                        ctx.set_fill_style(&JsValue::from_str("#1e40af"));
                        ctx.set_font("12px sans-serif");
                        ctx.set_text_align("center");
                        ctx.set_text_baseline("alphabetic");
                        ctx.fill_text(season_text, fixed_w / 2.0, screen_y + 18.0)
                            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
                    }
                    y_cursor += row_height;
                    if y_cursor - row_height > self.scroll_y + viewport_h {
                        break;
                    }
                }
            }
            if y_cursor > self.scroll_y + viewport_h {
                break;
            }
        }

        ctx.set_stroke_style(&JsValue::from_str("#94a3b8"));
        ctx.set_line_width(2.0);
        ctx.begin_path();
        ctx.move_to(fixed_w, viewport_y);
        ctx.line_to(fixed_w, viewport_y + viewport_h);
        ctx.stroke();
        ctx.set_line_width(1.0);
        ctx.restore();

        // ════════════════════════════════════════════════════════════════
        // PASS 3 — scrollable header
        // ════════════════════════════════════════════════════════════════
        ctx.save();
        ctx.begin_path();
        ctx.rect(fixed_w, start_y, viewport_w - fixed_w, total_header_h);
        ctx.clip();

        self.draw_scrollable_header(ctx, fixed_w, start_y, header_h1, header_h2, viewport_w)?;

        ctx.restore();

        // ════════════════════════════════════════════════════════════════
        // PASS 4 — pinned competition header
        // ════════════════════════════════════════════════════════════════
        ctx.set_fill_style(&JsValue::from_str("#e2e8f0"));
        ctx.fill_rect(0.0, start_y, fixed_w, total_header_h);
        ctx.set_stroke_style(&JsValue::from_str("#94a3b8"));
        ctx.set_line_width(1.0);
        ctx.stroke_rect(0.5, start_y + 0.5, fixed_w - 1.0, total_header_h - 1.0);
        ctx.set_fill_style(&JsValue::from_str("#1e293b"));
        ctx.set_font("bold 14px sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.fill_text(FIXED_COL, fixed_w / 2.0, start_y + total_header_h / 2.0)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
        ctx.set_stroke_style(&JsValue::from_str("#94a3b8"));
        ctx.set_line_width(2.0);
        ctx.begin_path();
        ctx.move_to(fixed_w, start_y);
        ctx.line_to(fixed_w, start_y + total_header_h);
        ctx.stroke();
        ctx.set_line_width(1.0);

        // ── Resize handles ──────────────────────────────────────────────
        ctx.set_stroke_style(&JsValue::from_str("#64748b"));
        ctx.set_line_width(2.0);

        // fixed col border
        ctx.begin_path();
        ctx.move_to(fixed_w, start_y + 6.0);
        ctx.line_to(fixed_w, start_y + total_header_h - 6.0);
        ctx.stroke();

        // scrollable col borders
        for (vc, sx, cw) in &col_positions {
            if vc.key == FIXED_COL {
                continue;
            }
            let border_x = sx + cw;
            if border_x > fixed_w && border_x < viewport_w {
                let top_y = start_y + header_h1 + 6.0;
                let bot_y = start_y + total_header_h - 6.0;
                ctx.begin_path();
                ctx.move_to(border_x, top_y);
                ctx.line_to(border_x, bot_y);
                ctx.stroke();
            }
        }
        ctx.set_line_width(1.0);

        // ── Vertical scrollbar ─────────────────────────────────────────
        let thumb = self.scrollbar_thumb_rect();
        if thumb[2] > 0.0 {
            ctx.set_fill_style(&JsValue::from_str("#f1f5f9"));
            ctx.fill_rect(width - scroll_bar_w, viewport_y, scroll_bar_w, viewport_h);
            ctx.set_fill_style(&JsValue::from_str("#94a3b8"));
            ctx.begin_path();
            ctx.round_rect_with_f64(thumb[0] + 1.0, thumb[1], thumb[2], thumb[3], 4.0)?;
            ctx.fill();
        }

        // ── Horizontal scrollbar ───────────────────────────────────────
        let hthumb = self.hscrollbar_thumb_rect();
        if hthumb[2] > 0.0 {
            ctx.set_fill_style(&JsValue::from_str("#f1f5f9"));
            ctx.fill_rect(0.0, height - scroll_bar_h, viewport_w, scroll_bar_h);
            ctx.set_fill_style(&JsValue::from_str("#94a3b8"));
            ctx.begin_path();
            ctx.round_rect_with_f64(hthumb[0] + 1.0, hthumb[1], hthumb[2], hthumb[3], 4.0)?;
            ctx.fill();
        }

        Ok(())
    }
}

// ─── Private helpers ─────────────────────────────────────────────────────────

impl PivotEngine {
    fn rebuild_groups(&mut self) {
        let mut order: Vec<String> = Vec::new();
        let mut map: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, row) in self.data.iter().enumerate() {
            let comp = row.fields.get(FIXED_COL).cloned().unwrap_or_default();
            if !map.contains_key(&comp) {
                order.push(comp.clone());
            }
            map.entry(comp).or_default().push(idx);
        }

        log_to_console(&map);

        let mut groups: Vec<Group> = Vec::with_capacity(order.len());
        for comp in order {
            let mut row_indices = map.remove(&comp).unwrap_or_default();
            log_to_console(&row_indices);
            row_indices.sort_by(|&a, &b| {
                let sa = self.data[a]
                    .fields
                    .get(GROUP_COL)
                    .cloned()
                    .unwrap_or_default();
                let sb = self.data[b]
                    .fields
                    .get(GROUP_COL)
                    .cloned()
                    .unwrap_or_default();
                match (sa.parse::<i64>(), sb.parse::<i64>()) {
                    (Ok(na), Ok(nb)) => na.cmp(&nb),
                    _ => sa.cmp(&sb),
                }
            });
            groups.push(Group {
                competition: comp,
                row_indices,
            });
        }
        self.groups = groups;
        log_to_console(&self.groups);
        // Build col_groups
        let mut winner_members: Vec<String> = {
            let mut set = HashSet::new();
            for row in &self.data {
                if let Some(v) = row.fields.get(WINNER_COL) {
                    if !v.is_empty() {
                        set.insert(v.clone());
                    }
                }
            }
            let mut v: Vec<String> = set.into_iter().collect();
            v.sort();
            v
        };
        let mut runner_up_members: Vec<String> = {
            let mut set = HashSet::new();
            for row in &self.data {
                if let Some(v) = row.fields.get(RUNNER_UP_COL) {
                    if !v.is_empty() {
                        set.insert(v.clone());
                    }
                }
            }
            let mut v: Vec<String> = set.into_iter().collect();
            v.sort();
            v
        };

        self.col_groups = vec![
            ColGroup {
                key: WINNER_COL.to_string(),
                label: "Winner".to_string(),
                expanded: false,
                members: winner_members,
            },
            ColGroup {
                key: RUNNER_UP_COL.to_string(),
                label: "Runner Up".to_string(),
                expanded: false,
                members: runner_up_members,
            },
        ];
        log_to_console(&self.col_groups);
    }

    fn compute_columns_and_widths(
        &mut self,
        ctx: &CanvasRenderingContext2d,
    ) -> Result<(), JsValue> {
        // columns = [FIXED_COL, GROUP_COL] + all winner members + all runner_up members
        let mut columns = vec![FIXED_COL.to_string(), GROUP_COL.to_string()];
        for cg in &self.col_groups {
            columns.push(cg.key.clone()); // header width slot
            for m in &cg.members {
                columns.push(format!("{}::{}", cg.key, m));
            }
        }
        self.columns = columns;

        ctx.set_font("bold 14px sans-serif");
        let mut widths: Vec<f64> = self
            .columns
            .iter()
            .map(|col| {
                let label = if col.contains("::") {
                    col.split("::").nth(1).unwrap_or(col).to_string()
                } else {
                    col.clone()
                };
                Ok(ctx.measure_text(&label)?.width() + 20.0)
            })
            .collect::<Result<Vec<_>, JsValue>>()?;

        // Sample rows for data widths — for member cols the value is "1" or ""
        ctx.set_font("12px sans-serif");
        for row in self.data.iter().take(200) {
            for (i, col) in self.columns.iter().enumerate() {
                let text = if col.contains("::") {
                    let parts: Vec<&str> = col.splitn(2, "::").collect();
                    let group_key = parts[0];
                    let member = parts[1];
                    if row
                        .fields
                        .get(group_key)
                        .map(|v| v == member)
                        .unwrap_or(false)
                    {
                        "1"
                    } else {
                        ""
                    }
                } else {
                    row.fields.get(col).map(|s| s.as_str()).unwrap_or("")
                };
                let w = ctx.measure_text(text)?.width() + 20.0;
                if w > widths[i] {
                    widths[i] = w;
                }
            }
        }
        // Member cols: cap at reasonable width
        for (i, col) in self.columns.iter().enumerate() {
            if col.contains("::") {
                widths[i] = widths[i].max(30.0).min(80.0);
            }
        }
        self.col_widths = widths;
        Ok(())
    }

    fn fixed_col_width(&self) -> f64 {
        self.width_idx_for(FIXED_COL)
            .pipe(|i| self.col_widths.get(i).copied().unwrap_or(120.0))
    }

    fn width_idx_for(&self, col: &str) -> usize {
        self.columns.iter().position(|c| c == col).unwrap_or(0)
    }

    /// Build visible cols list: [competition, season, (winner_header + winner_members?), (runner_up_header + runner_up_members?)]
    fn visible_cols(&self) -> Vec<VisCol> {
        let mut result = Vec::new();

        // competition (pinned)
        result.push(VisCol {
            key: FIXED_COL.to_string(),
            label: FIXED_COL.to_string(),
            width_idx: self.width_idx_for(FIXED_COL),
            col_group_idx: None,
        });

        // season
        result.push(VisCol {
            key: GROUP_COL.to_string(),
            label: GROUP_COL.to_string(),
            width_idx: self.width_idx_for(GROUP_COL),
            col_group_idx: None,
        });

        // col groups
        for (cgi, cg) in self.col_groups.iter().enumerate() {
            // header slot
            result.push(VisCol {
                key: cg.key.clone(),
                label: cg.label.clone(),
                width_idx: self.width_idx_for(&cg.key),
                col_group_idx: Some(cgi),
            });

            if cg.expanded {
                for m in &cg.members {
                    let col_key = format!("{}::{}", cg.key, m);
                    result.push(VisCol {
                        key: col_key.clone(),
                        label: m.clone(),
                        width_idx: self.width_idx_for(&col_key),
                        col_group_idx: Some(cgi),
                    });
                }
            }
        }
        result
    }

    /// Build (VisCol, screen_x, width) for all visible cols
    fn build_col_screen_positions(&self, fixed_w: f64) -> Vec<(VisCol, f64, f64)> {
        let vcs = self.visible_cols();
        let mut result = Vec::with_capacity(vcs.len());
        let mut x = fixed_w - self.scroll_x;

        for vc in vcs {
            let cw = self.col_widths[vc.width_idx];
            if vc.key == FIXED_COL {
                result.push((vc, 0.0, fixed_w));
            } else {
                result.push((vc, x, cw));
                x += cw;
            }
        }
        result
    }

    /// Get display value for a cell given a VisCol
    fn get_cell_value(&self, row: &DataRow, vc: &VisCol) -> String {
        if vc.key.contains("::") {
            let parts: Vec<&str> = vc.key.splitn(2, "::").collect();
            let group_key = parts[0];
            let member = parts[1];
            if row
                .fields
                .get(group_key)
                .map(|v| v == member)
                .unwrap_or(false)
            {
                "●".to_string()
            } else {
                String::new()
            }
        } else {
            row.fields.get(&vc.key).cloned().unwrap_or_default()
        }
    }

    fn col_group_header_width(&self, cgi: usize) -> f64 {
        let cg = &self.col_groups[cgi];
        let header_w = self
            .col_widths
            .get(self.width_idx_for(&cg.key))
            .copied()
            .unwrap_or(80.0);
        if cg.expanded {
            let members_w: f64 = cg
                .members
                .iter()
                .map(|m| {
                    let key = format!("{}::{}", cg.key, m);
                    self.col_widths
                        .get(self.width_idx_for(&key))
                        .copied()
                        .unwrap_or(40.0)
                })
                .sum();
            header_w + members_w
        } else {
            header_w
        }
    }

    fn draw_scrollable_header(
        &self,
        ctx: &CanvasRenderingContext2d,
        fixed_w: f64,
        start_y: f64,
        header_h1: f64,
        header_h2: f64,
        viewport_w: f64,
    ) -> Result<(), JsValue> {
        let sub_y = start_y + header_h1;
        let col_positions = self.build_col_screen_positions(fixed_w);

        // ── Row 1: season col header + col-group headers ──────────────
        // season header (row 1 + row 2 merged)
        let season_wi = self.width_idx_for(GROUP_COL);
        let season_w = self.col_widths[season_wi];
        // find screen x of season col
        let season_sx = col_positions
            .iter()
            .find(|(vc, _, _)| vc.key == GROUP_COL)
            .map(|(_, sx, _)| *sx)
            .unwrap_or(fixed_w);

        // season: spans both rows
        ctx.set_fill_style(&JsValue::from_str("#e2e8f0"));
        ctx.fill_rect(season_sx, start_y, season_w, header_h1 + header_h2);
        ctx.set_stroke_style(&JsValue::from_str("#94a3b8"));
        ctx.set_line_width(1.0);
        ctx.stroke_rect(
            season_sx + 0.5,
            start_y + 0.5,
            season_w - 1.0,
            header_h1 + header_h2 - 1.0,
        );
        ctx.set_fill_style(&JsValue::from_str("#1e293b"));
        ctx.set_font("bold 13px sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.fill_text(
            GROUP_COL,
            season_sx + season_w / 2.0,
            start_y + (header_h1 + header_h2) / 2.0,
        )
        .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        // Col group headers
        for (cgi, cg) in self.col_groups.iter().enumerate() {
            let span_w = self.col_group_header_width(cgi);
            // find screen x of this group's header col
            let group_sx = col_positions
                .iter()
                .find(|(vc, _, _)| vc.key == cg.key && vc.col_group_idx == Some(cgi))
                .map(|(_, sx, _)| *sx)
                .unwrap_or(0.0);

            let header_col_w = self.col_widths[self.width_idx_for(&cg.key)];
            let is_exp = cg.expanded;

            // Row 1 background (spans full group)
            let row1_fill = if cgi % 2 == 0 { "#dbeafe" } else { "#dcfce7" };
            let row1_stroke = if cgi % 2 == 0 { "#93c5fd" } else { "#86efac" };
            ctx.set_fill_style(&JsValue::from_str(row1_fill));
            ctx.fill_rect(group_sx, start_y, span_w, header_h1);
            ctx.set_stroke_style(&JsValue::from_str(row1_stroke));
            ctx.set_line_width(1.0);
            ctx.stroke_rect(group_sx + 0.5, start_y + 0.5, span_w - 1.0, header_h1 - 1.0);

            // Toggle button
            let btn_size = 16.0;
            let btn_x = group_sx + 4.0;
            let btn_y = start_y + (header_h1 - btn_size) / 2.0;
            let btn_fill = if is_exp { "#bfdbfe" } else { "#e0f2fe" };
            let btn_stroke = if is_exp { "#3b82f6" } else { "#7dd3fc" };
            ctx.set_fill_style(&JsValue::from_str(btn_fill));
            ctx.begin_path();
            ctx.round_rect_with_f64(btn_x, btn_y, btn_size, btn_size, 3.0)?;
            ctx.fill();
            ctx.set_stroke_style(&JsValue::from_str(btn_stroke));
            ctx.stroke();
            let arrow_color = if is_exp { "#1d4ed8" } else { "#0369a1" };
            ctx.set_fill_style(&JsValue::from_str(arrow_color));
            ctx.set_font("bold 11px sans-serif");
            ctx.set_text_align("center");
            ctx.set_text_baseline("middle");
            let arrow = if is_exp { "◀" } else { "▶" };
            ctx.fill_text(arrow, btn_x + btn_size / 2.0, start_y + header_h1 / 2.0)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

            // Label
            let label_fill = if cgi % 2 == 0 { "#1e3a8a" } else { "#14532d" };
            ctx.set_fill_style(&JsValue::from_str(label_fill));
            ctx.set_font("bold 13px sans-serif");
            ctx.set_text_align("center");
            ctx.set_text_baseline("middle");
            let btn_end = btn_x + btn_size + 4.0;
            let label_remaining = span_w - (btn_end - group_sx);
            ctx.fill_text(
                &cg.label,
                btn_end + label_remaining / 2.0,
                start_y + header_h1 / 2.0,
            )
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

            // Row 2
            let row2_fill = if cgi % 2 == 0 { "#eff6ff" } else { "#f0fdf4" };
            ctx.set_fill_style(&JsValue::from_str(row2_fill));
            ctx.fill_rect(group_sx, sub_y, span_w, header_h2);
            ctx.set_stroke_style(&JsValue::from_str(row1_stroke));
            ctx.set_line_width(1.0);
            ctx.stroke_rect(group_sx + 0.5, sub_y + 0.5, span_w - 1.0, header_h2 - 1.0);

            // Row 2 sub-cols: header col label
            ctx.set_fill_style(&JsValue::from_str(label_fill));
            ctx.set_font("bold 11px sans-serif");
            ctx.set_text_align("center");
            ctx.set_text_baseline("middle");
            ctx.fill_text(
                &cg.key,
                group_sx + header_col_w / 2.0,
                sub_y + header_h2 / 2.0,
            )
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

            // Row 2 member sub-cols (if expanded)
            if is_exp {
                let mut mx = group_sx + header_col_w;
                for (mi, member) in cg.members.iter().enumerate() {
                    let col_key = format!("{}::{}", cg.key, member);
                    let mw = self
                        .col_widths
                        .get(self.width_idx_for(&col_key))
                        .copied()
                        .unwrap_or(40.0);

                    // separator
                    ctx.set_stroke_style(&JsValue::from_str(row1_stroke));
                    ctx.set_line_width(1.0);
                    ctx.begin_path();
                    ctx.move_to(mx, sub_y + 2.0);
                    ctx.line_to(mx, sub_y + header_h2 - 2.0);
                    ctx.stroke();

                    ctx.set_fill_style(&JsValue::from_str(if cgi % 2 == 0 {
                        "#f0f9ff"
                    } else {
                        "#f0fdf4"
                    }));
                    ctx.fill_rect(mx + 1.0, sub_y + 1.0, mw - 1.0, header_h2 - 2.0);

                    ctx.set_fill_style(&JsValue::from_str(if cgi % 2 == 0 {
                        "#0c4a6e"
                    } else {
                        "#14532d"
                    }));
                    ctx.set_font("bold 10px sans-serif");
                    ctx.set_text_align("center");
                    ctx.set_text_baseline("middle");
                    ctx.fill_text(member, mx + mw / 2.0, sub_y + header_h2 / 2.0)
                        .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
                    mx += mw;
                }
            }
        }
        Ok(())
    }

    fn draw_empty_message(&self, ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
        let canvas = ctx.canvas().unwrap();
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;
        ctx.set_fill_style(&JsValue::from_str("#ffffff"));
        ctx.fill_rect(0.0, 0.0, width, height);
        ctx.set_fill_style(&JsValue::from_str("#999999"));
        ctx.set_font("16px sans-serif");
        let msg = "No data to display";
        let text_width = ctx.measure_text(msg)?.width();
        ctx.fill_text(msg, (width - text_width) / 2.0, height / 2.0)
            .map_err(|e| JsValue::from_str(&format!("Text error: {:?}", e)))?;
        Ok(())
    }
}

// tiny pipe helper so fixed_col_width is ergonomic
trait Pipe: Sized {
    fn pipe<F: FnOnce(Self) -> R, R>(self, f: F) -> R {
        f(self)
    }
}
impl Pipe for usize {}
