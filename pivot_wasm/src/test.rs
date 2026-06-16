use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use web_sys::CanvasRenderingContext2d;

const FIXED_COL: &str = "competition";
const GROUP_COL: &str = "season";
const WINNER_COL: &str = "winner";
const RUNNER_UP_COL: &str = "runner_up";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DataRow {
    pub fields: HashMap<string, string>,
}

#[derive(Clone, Debug)]
struct Group {
    competition: String,
    row_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
struct ColGroup {
    key: String,
    label: String,
    expanded: bool,
    members: Vec<String>,
}

#[wasm_bingen]
pub struct PivotEngine {
    data: Vec<DataRow>,
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

#[derive(Clone, Debug)]
struct VisCol {
    key: String,
    lable: String,
    width_idx: usize,
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

    pub fn set_scroll_y(&mut self, scroll_y: f64) {
        let max_scroll = self.total_content_height() - self.viewport_height;
        self.scroll_y = scroll_y.max(0.0).min(max_scroll.max(0.0));
    }

    pub fn get_scroll_y(&self) -> f64 {
        self.scroll_y
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

    pub fn toggle_row(&mut self, row_idx: usize) {
        if self.expanded_rows.contains(&row_idx) {
            self.expanded_rows.remove(&row_idx);
        } else {
            self.expanded_rows.insert(row_idx);
        }
    }
}

impl PivotEngine {
    fn rebuild_groups(&mut self) {
        let mut order: Vec<String> = Vec::new();
        let mut map: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, row) in self.data.iter().enumerate() {
            let comp = row.fields.get(FIXED_COL).cloned().unwrap_or_default();
            if !map.contains_key(&comp) {
                order.push(comp.clone());
            }
        }

        let mut groups: Vec<Group> = Vec::with_capacity(order.len());
        for comp in order {
            let mut row_indices = map.remove(&comp).unwrap_or_default();
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
                competitions: comp,
                row_indices,
            });
        }
        self.group = groups;
        let mut winner_set = HashSet::new();
        let mut runner_up_set = HashSet::new();

        for row in &self.data {
            // Обработка WINNER_COL
            if let Some(v) = row.fields.get(WINNER_COL) {
                let trimmed = v.trim();
                if !trimmed.is_empty() {
                    winner_set.insert(trimmed.to_string());
                }
            }
            // Обработка RUNNER_UP_COL
            if let Some(v) = row.fields.get(RUNNER_UP_COL) {
                let trimmed = v.trim();
                if !trimmed.is_empty() {
                    runner_up_set.insert(trimmed.to_string());
                }
            }
        }

        // Преобразование и сортировка
        let mut winner_members: Vec<String> = winner_set.into_iter().collect();
        winner_members.sort();

        let mut runner_up_members: Vec<String> = runner_up_set.into_iter().collect();
        runner_up_members.sort();

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
    }
}
