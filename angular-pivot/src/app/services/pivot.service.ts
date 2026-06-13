import { Injectable, signal } from "@angular/core";
import { WasmService } from "./wasm.service";
import { DataRow } from "../models/pivot.models";

@Injectable({ providedIn: "root" })
export class PivotService {
  private engine: any = null;

  data = signal<DataRow[]>([]);
  availableFields = signal<string[]>([]);
  isComputing = signal(false);
  error = signal<string | null>(null);

  constructor(private wasmService: WasmService) {}

  setData(rows: DataRow[]): void {
    this.data.set(rows);
    const fields = rows.length > 0 ? Object.keys(rows[0].fields) : [];
    this.availableFields.set(fields);
    if (this.engine) {
      this.engine.update_data(rows);
    }
  }

  async initEngine(): Promise<void> {
    this.engine = this.wasmService.createEngine(this.data());
  }

  set_viewport(width: number, height: number) {
    this.engine.set_viewport(width, height);
  }

  set_scroll_y(v: number) {
    this.engine.set_scroll_y(v);
  }
  get_scroll_y(): number {
    return this.engine.get_scroll_y();
  }
  set_scroll_x(v: number) {
    this.engine.set_scroll_x(v);
  }
  get_scroll_x(): number {
    return this.engine.get_scroll_x();
  }

  scrollbar_thumb_rect(): number[] {
    return Array.from(this.engine.scrollbar_thumb_rect());
  }
  hscrollbar_thumb_rect(): number[] {
    return Array.from(this.engine.hscrollbar_thumb_rect());
  }

  total_content_height(): number {
    return this.engine.total_content_height();
  }

  render(ctx: CanvasRenderingContext2D): void {
    if (!this.engine) return;
    this.engine.render(ctx);
  }

  /** Returns col_group index or -1 */
  hit_test_col_group_toggle(px: number, py: number): number {
    return this.engine.hit_test_col_group_toggle(px, py);
  }

  toggle_col_group(groupIdx: number): void {
    this.engine.toggle_col_group(groupIdx);
  }

  is_col_group_expanded(groupIdx: number): boolean {
    return this.engine.is_col_group_expanded(groupIdx);
  }

  col_group_count(): number {
    return this.engine.col_group_count();
  }

  hit_test_row(
    py: number,
    scroll_y: number,
    start_y: number,
    header_h: number,
    row_height: number,
  ): number {
    return this.engine.hit_test_row(
      py,
      scroll_y,
      start_y,
      header_h,
      row_height,
    );
  }

  toggle_row(rowIdx: number) {
    this.engine.toggle_row(rowIdx);
  }

  get_col_widths(): number[] {
    return Array.from(this.engine.get_col_widths());
  }

  set_col_width(colIdx: number, width: number) {
    this.engine.set_col_width(colIdx, width);
  }

  get_visible_col_widths(): number[] {
    return Array.from(this.engine.get_visible_col_widths());
  }

  set_visible_col_width(visibleIdx: number, width: number) {
    this.engine.set_visible_col_width(visibleIdx, width);
  }

  swap_visible_columns(from: number, to: number): void {
    this.engine.swap_visible_columns(from, to);
  }

  get_col_name_at_visible_idx(idx: number): string {
    return this.engine.get_col_name_at_visible_idx(idx);
  }
}
