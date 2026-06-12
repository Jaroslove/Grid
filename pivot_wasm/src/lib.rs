use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use web_sys::CanvasRenderingContext2d;
use web_sys::console;

const FIXED_COL: &str = "competition";
const GROUP_COL: &str = "season";

// ─── Data Structures ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DataRow {
    pub fields: HashMap<String, String>,
}

// ─── Core Engine ────────────────────────────────────────────────────────────

#[wasm_bindgen]
pub struct PivotEngine {
    data: Vec<DataRow>,
    columns: Vec<String>,
    col_widths: Vec<f64>,
    expanded_rows: HashSet<usize>,
    expanded_col_group: bool,
    scroll_y: f64,
    scroll_x: f64,
    viewport_height: f64,
    viewport_width: f64,
    canvas_width: f64,
    canvas_height: f64,
}

#[wasm_bindgen]
impl PivotEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(data: JsValue) -> Result<PivotEngine, JsValue> {
        let rows: Vec<DataRow> = serde_wasm_bindgen::from_value(data)
            .map_err(|e| JsValue::from_str(&format!("Invalid data: {}", e)))?;
        console::log_1(&"new fn".into());
        let js_value = to_value(&rows).unwrap();
        console::log_1(&js_value);
        Ok(PivotEngine {
            data: rows,
            columns: Vec::new(),
            col_widths: Vec::new(),
            expanded_rows: HashSet::new(),
            expanded_col_group: false,
            scroll_y: 0.0,
            scroll_x: 0.0,
            viewport_height: 0.0,
            viewport_width: 0.0,
            canvas_width: 0.0,
            canvas_height: 0.0,
        })
    }

    pub fn set_viewport(&mut self, width: f64, height: f64) {
        self.canvas_width = width;
        self.canvas_height = height;
        self.viewport_height = height - 54.0 - 10.0; // headers + hscroll bar
        self.viewport_width = width - 10.0; // minus vscroll bar
    }

    pub fn total_content_height(&self) -> f64 {
        let row_height = 25.0;
        let expanded = self.expanded_rows.len();
        (self.data.len() + expanded) as f64 * row_height
    }

    pub fn set_scroll_x(&mut self, scroll_x: f64) {
        let total_w: f64 = self
            .visible_data_columns()
            .iter()
            .map(|col| {
                let i = self.columns.iter().position(|c| c == col).unwrap();
                self.col_widths[i]
            })
            .sum();
        let max_scroll = (total_w - self.viewport_width).max(0.0);
        self.scroll_x = scroll_x.max(0.0).min(max_scroll);
    }

    pub fn get_scroll_x(&self) -> f64 {
        self.scroll_x
    }

    pub fn hscrollbar_thumb_rect(&self) -> Vec<f64> {
        let total_w: f64 = self
            .visible_data_columns()
            .iter()
            .map(|col| {
                let i = self.columns.iter().position(|c| c == col).unwrap();
                self.col_widths[i]
            })
            .sum();
        let track_w = self.viewport_width;
        if total_w <= track_w {
            return vec![0.0, 0.0, 0.0, 0.0];
        }
        let thumb_w = (track_w / total_w * track_w).max(30.0);
        let thumb_x = (self.scroll_x / total_w) * track_w;
        // [x, y, width, height]
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

    pub fn set_col_width(&mut self, col_idx: usize, width: f64) {
        if col_idx < self.col_widths.len() {
            self.col_widths[col_idx] = width.max(30.0); // min 30px
        }
    }

    pub fn get_col_widths(&self) -> Vec<f64> {
        self.col_widths.clone()
    }

    pub fn col_count(&self) -> usize {
        self.col_widths.len()
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
        for (i, _) in self.data.iter().enumerate() {
            if relative_y >= y && relative_y < y + row_height {
                return i as i32;
            }
            y += row_height;
            if self.expanded_rows.contains(&i) {
                y += row_height; // skip expanded row
            }
        }
        -1
    }

    pub fn update_data(&mut self, data: JsValue) -> Result<(), JsValue> {
        self.data = serde_wasm_bindgen::from_value(data)
            .map_err(|e| JsValue::from_str(&format!("Invalid data: {}", e)))?;
        // Clear cached columns/widths – they will be recomputed on next render
        self.columns.clear();
        self.col_widths.clear();
        Ok(())
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

        let data_cols = self.visible_data_columns();
        let children = self
            .season_children()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let fixed_idx = self.columns.iter().position(|c| c == FIXED_COL).unwrap();
        let group_idx = self.columns.iter().position(|c| c == GROUP_COL).unwrap();
        let fixed_w = self.col_widths[fixed_idx];
        let group_w = self.col_widths[group_idx];

        let children_total_w: f64 = if self.expanded_col_group {
            children
                .iter()
                .map(|c| {
                    let i = self.columns.iter().position(|x| x == c).unwrap();
                    self.col_widths[i]
                })
                .sum()
        } else {
            0.0
        };
        let season_span_w = group_w + children_total_w;

        // ── Clear ────────────────────────────────────────────────────────────
        ctx.set_fill_style(&JsValue::from_str("#ffffff"));
        ctx.fill_rect(0.0, 0.0, width, height);

        // ── Helper: compute screen_x for each visible col (scrollable cols offset by scroll_x) ──
        // Returns (col_name, screen_x, col_width) for all visible cols
        let col_screen_positions: Vec<(String, f64, f64)> = {
            let mut result = Vec::new();
            // fixed col always at 0
            result.push((FIXED_COL.to_string(), 0.0, fixed_w));
            // scrollable cols start at fixed_w, offset by scroll_x
            let mut x = fixed_w - self.scroll_x;
            for col in &data_cols {
                if col == FIXED_COL {
                    continue;
                }
                let ci = self.columns.iter().position(|c| c == col).unwrap();
                let cw = self.col_widths[ci];
                result.push((col.clone(), x, cw));
                x += cw;
            }
            result
        };

        // ════════════════════════════════════════════════════════════════════
        // PASS 1 — scrollable data cells (clipped right of fixed col)
        // ════════════════════════════════════════════════════════════════════
        ctx.save();
        ctx.begin_path();
        ctx.rect(fixed_w, viewport_y, viewport_w - fixed_w, viewport_h);
        ctx.clip();

        let mut y_cursor = 0.0;
        for (row_idx, row) in self.data.iter().enumerate() {
            let is_expanded = self.expanded_rows.contains(&row_idx);
            let row_top = y_cursor;
            let row_bottom = row_top + row_height;

            if row_bottom < self.scroll_y {
                y_cursor += row_height;
                if is_expanded {
                    y_cursor += row_height;
                }
                continue;
            }
            if row_top > self.scroll_y + viewport_h {
                break;
            }

            let screen_y = viewport_y + (row_top - self.scroll_y);
            let fill_color = if row_idx % 2 == 0 {
                "#ffffff"
            } else {
                "#fafafa"
            };

            // Row background
            ctx.set_fill_style(&JsValue::from_str(fill_color));
            ctx.fill_rect(fixed_w, screen_y, viewport_w - fixed_w, row_height);

            // Draw each scrollable col
            for (col, sx, cw) in &col_screen_positions {
                if col == FIXED_COL {
                    continue;
                }
                // skip if fully outside visible area
                if sx + cw <= fixed_w || *sx >= viewport_w {
                    continue;
                }

                ctx.set_stroke_style(&JsValue::from_str("#cccccc"));
                ctx.set_line_width(1.0);
                ctx.stroke_rect(*sx, screen_y, *cw, row_height);

                let cell_text = row.fields.get(col).map(|s| s.as_str()).unwrap_or("");
                ctx.set_fill_style(&JsValue::from_str("#000000"));
                ctx.set_font("12px sans-serif");
                ctx.set_text_align("center");
                ctx.set_text_baseline("alphabetic");
                ctx.fill_text(cell_text, sx + cw / 2.0, screen_y + 18.0)
                    .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
            }
            y_cursor += row_height;

            // ── Expanded duplicate row ───────────────────────────────────────
            if is_expanded {
                let exp_y = viewport_y + (y_cursor - self.scroll_y);
                if exp_y < viewport_y + viewport_h {
                    ctx.set_fill_style(&JsValue::from_str("#eff6ff"));
                    ctx.fill_rect(fixed_w, exp_y, viewport_w - fixed_w, row_height);

                    for (col, sx, cw) in &col_screen_positions {
                        if col == FIXED_COL {
                            continue;
                        }
                        if sx + cw <= fixed_w || *sx >= viewport_w {
                            continue;
                        }

                        ctx.set_stroke_style(&JsValue::from_str("#bfdbfe"));
                        ctx.set_line_width(1.0);
                        ctx.stroke_rect(*sx, exp_y, *cw, row_height);

                        let cell_text = row.fields.get(col).map(|s| s.as_str()).unwrap_or("");
                        ctx.set_fill_style(&JsValue::from_str("#1e40af"));
                        ctx.set_font("12px sans-serif");
                        ctx.set_text_align("center");
                        ctx.set_text_baseline("alphabetic");
                        ctx.fill_text(cell_text, sx + cw / 2.0, exp_y + 18.0)
                            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
                    }
                }
                y_cursor += row_height;
            }
        }
        ctx.restore();

        // ════════════════════════════════════════════════════════════════════
        // PASS 2 — pinned competition column (drawn on top of pass 1)
        // ════════════════════════════════════════════════════════════════════
        ctx.save();
        ctx.begin_path();
        ctx.rect(0.0, viewport_y, fixed_w, viewport_h);
        ctx.clip();

        let mut y_cursor = 0.0;
        for (row_idx, row) in self.data.iter().enumerate() {
            let is_expanded = self.expanded_rows.contains(&row_idx);
            let row_top = y_cursor;
            let row_bottom = row_top + row_height;

            if row_bottom < self.scroll_y {
                y_cursor += row_height;
                if is_expanded {
                    y_cursor += row_height;
                }
                continue;
            }
            if row_top > self.scroll_y + viewport_h {
                break;
            }

            let screen_y = viewport_y + (row_top - self.scroll_y);
            let fill_color = if row_idx % 2 == 0 {
                "#ffffff"
            } else {
                "#fafafa"
            };

            // Pinned cell background + border
            ctx.set_fill_style(&JsValue::from_str(fill_color));
            ctx.fill_rect(0.0, screen_y, fixed_w, row_height);
            ctx.set_stroke_style(&JsValue::from_str("#cccccc"));
            ctx.set_line_width(1.0);
            ctx.stroke_rect(0.0, screen_y, fixed_w, row_height);

            // Toggle button
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

            let cell_text = row.fields.get(FIXED_COL).map(|s| s.as_str()).unwrap_or("");
            ctx.set_fill_style(&JsValue::from_str("#000000"));
            ctx.set_font("12px sans-serif");
            let btn_end = btn_x + btn_size + 4.0;
            let remaining = fixed_w - btn_end;
            ctx.set_text_align("center");
            ctx.set_text_baseline("alphabetic");
            ctx.fill_text(cell_text, btn_end + remaining / 2.0, screen_y + 18.0)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

            y_cursor += row_height;

            // Expanded pinned cell — use exp_y not screen_y
            if is_expanded {
                let exp_y = viewport_y + (y_cursor - self.scroll_y);
                if exp_y < viewport_y + viewport_h {
                    ctx.set_fill_style(&JsValue::from_str("#eff6ff"));
                    ctx.fill_rect(0.0, exp_y, fixed_w, row_height);
                    ctx.set_stroke_style(&JsValue::from_str("#bfdbfe"));
                    ctx.set_line_width(1.0);
                    ctx.stroke_rect(0.0, exp_y, fixed_w, row_height);

                    let cell_text = row.fields.get(FIXED_COL).map(|s| s.as_str()).unwrap_or("");
                    ctx.set_fill_style(&JsValue::from_str("#1e40af"));
                    ctx.set_font("12px sans-serif");
                    ctx.set_text_align("center");
                    ctx.set_text_baseline("alphabetic");
                    ctx.fill_text(cell_text, fixed_w / 2.0, exp_y + 18.0)
                        .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
                }
                y_cursor += row_height;
            }
        }

        // Right-edge separator
        ctx.set_stroke_style(&JsValue::from_str("#94a3b8"));
        ctx.set_line_width(2.0);
        ctx.begin_path();
        ctx.move_to(fixed_w, viewport_y);
        ctx.line_to(fixed_w, viewport_y + viewport_h);
        ctx.stroke();
        ctx.set_line_width(1.0);
        ctx.restore();

        // ════════════════════════════════════════════════════════════════════
        // PASS 3 — scrollable header (clipped right of fixed col)
        // ════════════════════════════════════════════════════════════════════
        ctx.save();
        ctx.begin_path();
        ctx.rect(fixed_w, start_y, viewport_w - fixed_w, total_header_h);
        ctx.clip();

        let season_draw_x = fixed_w - self.scroll_x;
        let sub_y = start_y + header_h1;
        let is_exp = self.expanded_col_group;

        // season row 1
        ctx.set_fill_style(&JsValue::from_str("#dbeafe"));
        ctx.fill_rect(season_draw_x, start_y, season_span_w, header_h1);
        ctx.set_stroke_style(&JsValue::from_str("#93c5fd"));
        ctx.set_line_width(1.0);
        ctx.stroke_rect(
            season_draw_x + 0.5,
            start_y + 0.5,
            season_span_w - 1.0,
            header_h1 - 1.0,
        );

        // toggle button
        let btn_size = 16.0;
        let btn_x = season_draw_x + 4.0;
        let btn_y = start_y + (header_h1 - btn_size) / 2.0;

        ctx.set_fill_style(&JsValue::from_str(if is_exp {
            "#bfdbfe"
        } else {
            "#e0f2fe"
        }));
        ctx.begin_path();
        ctx.round_rect_with_f64(btn_x, btn_y, btn_size, btn_size, 3.0)?;
        ctx.fill();
        ctx.set_stroke_style(&JsValue::from_str(if is_exp {
            "#3b82f6"
        } else {
            "#7dd3fc"
        }));
        ctx.set_line_width(1.0);
        ctx.stroke();

        ctx.set_fill_style(&JsValue::from_str(if is_exp {
            "#1d4ed8"
        } else {
            "#0369a1"
        }));
        ctx.set_font("bold 11px sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        let arrow = if is_exp { "◀" } else { "▶" };
        ctx.fill_text(arrow, btn_x + btn_size / 2.0, start_y + header_h1 / 2.0)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        ctx.set_fill_style(&JsValue::from_str("#1e3a8a"));
        ctx.set_font("bold 14px sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        let btn_end = btn_x + btn_size + 4.0;
        let label_remaining = season_span_w - (btn_end - season_draw_x);
        ctx.fill_text(
            GROUP_COL,
            btn_end + label_remaining / 2.0,
            start_y + header_h1 / 2.0,
        )
        .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        // season row 2
        ctx.set_fill_style(&JsValue::from_str("#eff6ff"));
        ctx.fill_rect(season_draw_x, sub_y, season_span_w, header_h2);
        ctx.set_stroke_style(&JsValue::from_str("#bfdbfe"));
        ctx.set_line_width(1.0);
        ctx.stroke_rect(
            season_draw_x + 0.5,
            sub_y + 0.5,
            season_span_w - 1.0,
            header_h2 - 1.0,
        );

        ctx.set_fill_style(&JsValue::from_str("#1e3a8a"));
        ctx.set_font("bold 12px sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.fill_text(
            GROUP_COL,
            season_draw_x + group_w / 2.0,
            sub_y + header_h2 / 2.0,
        )
        .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        if self.expanded_col_group {
            ctx.set_stroke_style(&JsValue::from_str("#bfdbfe"));
            ctx.set_line_width(1.0);
            ctx.begin_path();
            ctx.move_to(season_draw_x + group_w, sub_y + 2.0);
            ctx.line_to(season_draw_x + group_w, sub_y + header_h2 - 2.0);
            ctx.stroke();

            let mut cx = season_draw_x + group_w;
            for (ci_idx, child) in children.iter().enumerate() {
                let ci = self.columns.iter().position(|c| c == child).unwrap();
                let cw = self.col_widths[ci];

                ctx.set_fill_style(&JsValue::from_str("#f0f9ff"));
                ctx.fill_rect(cx + 1.0, sub_y + 1.0, cw - 1.0, header_h2 - 2.0);

                if ci_idx < children.len() - 1 {
                    ctx.set_stroke_style(&JsValue::from_str("#bae6fd"));
                    ctx.set_line_width(1.0);
                    ctx.begin_path();
                    ctx.move_to(cx + cw, sub_y + 2.0);
                    ctx.line_to(cx + cw, sub_y + header_h2 - 2.0);
                    ctx.stroke();
                }

                ctx.set_fill_style(&JsValue::from_str("#0c4a6e"));
                ctx.set_font("bold 12px sans-serif");
                ctx.set_text_align("center");
                ctx.set_text_baseline("middle");
                ctx.fill_text(child, cx + cw / 2.0, sub_y + header_h2 / 2.0)
                    .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
                cx += cw;
            }
        }
        ctx.restore();

        // ════════════════════════════════════════════════════════════════════
        // PASS 4 — pinned competition header (always on top)
        // ════════════════════════════════════════════════════════════════════
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

        // Separator line between pinned and scrollable header
        ctx.set_stroke_style(&JsValue::from_str("#94a3b8"));
        ctx.set_line_width(2.0);
        ctx.begin_path();
        ctx.move_to(fixed_w, start_y);
        ctx.line_to(fixed_w, start_y + total_header_h);
        ctx.stroke();
        ctx.set_line_width(1.0);

        // ── Resize handles with per‑column header height ─────────────────────
        let header_row1_h = 30.0;
        let header_row2_h = 24.0;
        let total_header_h = header_row1_h + header_row2_h;

        let is_expanded = self.expanded_col_group;

        ctx.set_stroke_style(&JsValue::from_str("#64748b"));
        ctx.set_line_width(2.0);

        // Helper: given a column name, return (start_y, end_y) relative to canvas top
        let get_handle_ys = |col_name: &str| -> (f64, f64) {
            if col_name == FIXED_COL {
                // Competition column spans both header rows
                (start_y + 6.0, start_y + total_header_h - 6.0)
            } else {
                // All other columns (including parent "season" when expanded) – second row only
                (
                    start_y + header_row1_h + 6.0,
                    start_y + total_header_h - 6.0,
                )
            }
            // if col_name == FIXED_COL {
            //     // Competition column spans both header rows
            //     (start_y + 6.0, start_y + total_header_h - 6.0)
            // } else if is_expanded && col_name == GROUP_COL {
            //     // Season group column – only top row (merged big cell)
            //     (start_y + 6.0, start_y + header_row1_h - 6.0)
            // } else {
            //     // Child columns (or season when collapsed) – only second row
            //     (
            //         start_y + header_row1_h + 6.0,
            //         start_y + total_header_h - 6.0,
            //     )
            // }
        };

        // Fixed column right border
        let (top_y, bottom_y) = get_handle_ys(FIXED_COL);
        ctx.begin_path();
        ctx.move_to(fixed_w, top_y);
        ctx.line_to(fixed_w, bottom_y);
        ctx.stroke();

        // Scrollable columns
        for (col, sx, cw) in &col_screen_positions {
            console::log_1(&format!("scroll col: {}", col).into());
            if col == FIXED_COL {
                continue;
            }

            let border_x = sx + cw;
            if border_x > fixed_w && border_x < viewport_w {
                let (top_y, bottom_y) = get_handle_ys(col);
                console::log_1(&format!("top_y col: {}", top_y).into());
                console::log_1(&format!("bottom_y col: {}", bottom_y).into());
                ctx.begin_path();
                ctx.move_to(border_x, top_y);
                ctx.line_to(border_x, bottom_y);
                ctx.stroke();
            }
        }
        ctx.set_line_width(1.0);

        // ── Vertical scrollbar ───────────────────────────────────────────────
        let thumb = self.scrollbar_thumb_rect();
        if thumb[2] > 0.0 {
            ctx.set_fill_style(&JsValue::from_str("#f1f5f9"));
            ctx.fill_rect(width - scroll_bar_w, viewport_y, scroll_bar_w, viewport_h);
            ctx.set_fill_style(&JsValue::from_str("#94a3b8"));
            ctx.begin_path();
            ctx.round_rect_with_f64(thumb[0] + 1.0, thumb[1], thumb[2], thumb[3], 4.0)?;
            ctx.fill();
        }

        // ── Horizontal scrollbar ─────────────────────────────────────────────
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

    pub fn toggle_col_group(&mut self) {
        self.expanded_col_group = !self.expanded_col_group;
    }

    pub fn is_col_group_expanded(&self) -> bool {
        self.expanded_col_group
    }

    pub fn get_visible_col_widths(&self) -> Vec<f64> {
        self.visible_data_columns()
            .iter()
            .map(|col| {
                // console::log_1(&format!("visible_data_column = {}", col).into());
                let i = self.columns.iter().position(|c| c == col).unwrap();
                // console::log_1(&format!("visible_data_column_index = {}", i).into());
                let size = self.col_widths[i];
                // console::log_1(&format!("visible_data_column_size = {}", size).into());
                size
            })
            .collect()
    }

    pub fn set_visible_col_width(&mut self, visible_idx: usize, width: f64) {
        let visible = self.visible_data_columns();
        if let Some(col) = visible.get(visible_idx) {
            if let Some(i) = self.columns.iter().position(|c| c == col) {
                self.col_widths[i] = width.max(30.0);
            }
        }
    }

    /// Hit test for column group toggle button in header
    /// Returns true if px is within the "season" column toggle button
    pub fn hit_test_col_toggle(&self, px: f64, py: f64, _start_y: f64, _header_h: f64) -> bool {
        // Only in top header row
        if py < 0.0 || py > 30.0 {
            return false;
        }

        let fixed_idx = self.columns.iter().position(|c| c == FIXED_COL).unwrap();
        let fixed_w = self.col_widths[fixed_idx];

        // Button is at season_x + 4, width 16
        let season_x = fixed_w;
        let btn_x = season_x + 4.0;
        px >= btn_x && px <= btn_x + 16.0
    }

    pub fn swap_visible_columns(&mut self, from_visible_idx: usize, to_visible_idx: usize) {
        let visible = self.visible_data_columns();
        // Don't allow swapping with or swapping the fixed col
        if from_visible_idx == 0 || to_visible_idx == 0 {
            return;
        }
        if from_visible_idx == to_visible_idx {
            return;
        }
        if from_visible_idx >= visible.len() || to_visible_idx >= visible.len() {
            return;
        }

        let from_col = &visible[from_visible_idx].clone();
        let to_col = &visible[to_visible_idx].clone();

        // Find positions in self.columns and swap
        if let (Some(fi), Some(ti)) = (
            self.columns.iter().position(|c| c == from_col),
            self.columns.iter().position(|c| c == to_col),
        ) {
            self.columns.swap(fi, ti);
            self.col_widths.swap(fi, ti);
        }
    }

    pub fn get_col_name_at_visible_idx(&self, visible_idx: usize) -> String {
        self.visible_data_columns()
            .get(visible_idx)
            .cloned()
            .unwrap_or_default()
    }
}

// Private helpers
impl PivotEngine {
    /// Computes column names and their widths using the provided canvas context.
    /// No temporary canvas – the real rendering context is used for measurement.
    fn compute_columns_and_widths(
        &mut self,
        ctx: &CanvasRenderingContext2d,
    ) -> Result<(), JsValue> {
        // Collect all unique column names from all rows
        let mut col_set = HashSet::new();
        for row in &self.data {
            for key in row.fields.keys() {
                col_set.insert(key.clone());
            }
        }
        let mut columns: Vec<String> = col_set.into_iter().collect();
        columns.sort();
        self.columns = columns;

        // Measure header widths (bold 14px)
        ctx.set_font("bold 14px sans-serif");
        let mut widths: Vec<f64> = self
            .columns
            .iter()
            .map(|col| Ok(ctx.measure_text(col)?.width() + 20.0))
            .collect::<Result<Vec<_>, JsValue>>()?;

        // Measure cell widths (regular 12px)
        ctx.set_font("12px sans-serif");
        for row in &self.data {
            for (i, col) in self.columns.iter().enumerate() {
                let text = row.fields.get(col).map(|s| s.as_str()).unwrap_or("");
                let w = ctx.measure_text(text)?.width() + 20.0;
                if w > widths[i] {
                    widths[i] = w;
                }
            }
        }
        self.col_widths = widths;
        Ok(())
    }

    fn draw_empty_message(&self, ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
        let canvas = ctx.canvas().unwrap();
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;
        set_canvas_fill_color(ctx, "#ffffff");
        ctx.fill_rect(0.0, 0.0, width, height);
        set_canvas_fill_color(ctx, "#999999");
        ctx.set_font("16px sans-serif");
        let msg = "No data to display";
        let text_width = ctx.measure_text(msg)?.width();
        ctx.fill_text(msg, (width - text_width) / 2.0, height / 2.0)
            .map_err(|e| JsValue::from_str(&format!("Text error: {:?}", e)))?;
        Ok(())
    }

    fn season_children(&self) -> Vec<&String> {
        self.columns
            .iter()
            .filter(|c| c.as_str() != FIXED_COL && c.as_str() != GROUP_COL)
            .collect()
    }

    /// Flat ordered list of leaf columns actually rendered in data rows
    fn visible_data_columns(&self) -> Vec<String> {
        let mut result = vec![FIXED_COL.to_string(), GROUP_COL.to_string()];
        if self.expanded_col_group {
            for c in self.season_children() {
                result.push(c.clone());
            }
        }
        result
    }
}

fn set_canvas_fill_color(context: &CanvasRenderingContext2d, color: &str) {
    // 1. Get a reference to the context as a raw JsValue
    let context_val: &JsValue = context;

    // 2. Reflectively set the "fillStyle" property on the JS object
    js_sys::Reflect::set(
        context_val,
        &JsValue::from_str("fillStyle"),
        &JsValue::from_str(color),
    )
    .unwrap();
}
