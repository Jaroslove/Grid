export interface DataRow {
  fields: Record<string, string>;
}

export interface ValueConfig {
  field: string;
  aggregation: "sum" | "count" | "avg" | "min" | "max";
  label: string;
}

export interface FilterConfig {
  field: string;
  values: string[];
}

export interface PivotConfig {
  rows: string[];
  columns: string[];
  values: ValueConfig[];
  filters: FilterConfig[];
}

export interface PivotResult {
  row_headers: string[][];
  col_headers: string[][];
  data: (number | null)[][];
  row_totals: number[];
  col_totals: number[];
  grand_total: number;
  value_labels: string[];
}

export interface RenderConfig {
  cell_width: number;
  cell_height: number;
  header_bg: string;
  header_color: string;
  cell_bg: string;
  cell_bg_alt: string;
  border_color: string;
  total_bg: string;
  font_size: number;
  font_family: string;
  show_totals: boolean;
  highlight_color: string;
}

export interface HitTestResult {
  row: number;
  col: number;
  is_data: boolean;
}

export const DEFAULT_RENDER_CONFIG: RenderConfig = {
  cell_width: 120,
  cell_height: 32,
  header_bg: "#2563eb",
  header_color: "#ffffff",
  cell_bg: "#ffffff",
  cell_bg_alt: "#f1f5f9",
  border_color: "#e2e8f0",
  total_bg: "#dbeafe",
  font_size: 13,
  font_family: "Inter, sans-serif",
  show_totals: true,
  highlight_color: "#fef08a",
};
