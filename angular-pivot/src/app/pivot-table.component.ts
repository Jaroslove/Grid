import {
  Component,
  OnInit,
  OnDestroy,
  ViewChild,
  ElementRef,
  ChangeDetectionStrategy,
} from "@angular/core";
import { CommonModule } from "@angular/common";
import { FormsModule } from "@angular/forms";
import { PivotService } from "./services/pivot.service";
import { WasmService } from "./services/wasm.service";
import { populateDataRows } from "./data/sample-data";
import { HttpClient, HttpClientModule } from "@angular/common/http";

@Component({
  selector: "app-pivot-table",
  standalone: true,
  imports: [CommonModule, FormsModule, HttpClientModule],
  templateUrl: "./pivot-table.component.html",
  styleUrls: ["./pivot-table.component.scss"],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class PivotTableComponent implements OnInit, OnDestroy {
  @ViewChild("pivotCanvas") canvasRef!: ElementRef<HTMLCanvasElement>;

  canvasWidth = 1200;
  canvasHeight = 600;

  private ctx: CanvasRenderingContext2D | null = null;
  private animFrame: number | null = null;
  private destroyed = false;

  // ── Column resize state ──────────────────────────────────────────────────
  private drag = {
    active: false,
    colIdx: -1,
    startX: 0,
    startWidth: 0,
  };
  private hoverColBorder = -1;
  private readonly RESIZE_HIT_ZONE = 5;

  // ── Column swap drag state ───────────────────────────────────────────────
  private colSwap = {
    active: false,
    fromIdx: -1, // visible col index being dragged
    startX: 0,
    startY: 0,
    currentX: 0,
    targetIdx: -1, // visible col index under cursor
  };

  // ── Scroll drag state ────────────────────────────────────────────────────
  private scrollDrag = false;
  private scrollDragStartY = 0;
  private scrollDragStartScroll = 0;
  private hScrollDrag = false;
  private hScrollDragStartX = 0;
  private hScrollDragStartScroll = 0;

  constructor(
    public pivotSvc: PivotService,
    private wasmSvc: WasmService,
    private http: HttpClient,
  ) {}

  async ngOnInit(): Promise<void> {
    await this.wasmSvc.load();
    const parsedData = await this.http
      .get<any>("/assets/winners.json")
      .toPromise();
    const parsedDataUefa = await this.http
      .get<any>("/assets/winners_uefa.json")
      .toPromise();
    const dataRows = populateDataRows(parsedData);
    const dataRowsUefa = populateDataRows(parsedDataUefa);
    // const increased = Array.from({ length: 10 }, () => dataRows).flat();
    const combined = [...dataRows, ...dataRowsUefa];
    this.pivotSvc.setData(combined);
    await this.pivotSvc.initEngine();
    this.init();
    if (!this.ctx) return;
    this.pivotSvc.render(this.ctx);
  }

  ngOnDestroy(): void {
    this.destroyed = true;
    if (this.animFrame) cancelAnimationFrame(this.animFrame);
  }

  // ── Init ─────────────────────────────────────────────────────────────────
  init() {
    const canvas = this.canvasRef.nativeElement;
    this.ctx = canvas.getContext("2d")!;
    this.pivotSvc.set_viewport(canvas.width, canvas.height);
    this.scheduleRender();

    canvas.addEventListener(
      "wheel",
      (e: WheelEvent) => {
        e.preventDefault();
        if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
          this.pivotSvc.set_scroll_x(
            this.pivotSvc.get_scroll_x() + (e.deltaX || e.deltaY),
          );
        } else {
          this.pivotSvc.set_scroll_y(this.pivotSvc.get_scroll_y() + e.deltaY);
        }
        this.pivotSvc.render(this.ctx!);
      },
      { passive: false },
    );

    canvas.addEventListener("mousedown", (e) => this.onMouseDown(e));
    canvas.addEventListener("mousemove", (e) => this.onMouseMove(e));
    canvas.addEventListener("mouseup", (e) => this.onMouseUp(e));
    canvas.addEventListener("mouseleave", (e) => this.onMouseLeave(e));
    canvas.addEventListener("click", (e) => this.onCanvasClick(e));
  }

  scheduleRender(): void {
    if (this.animFrame) cancelAnimationFrame(this.animFrame);
    this.animFrame = requestAnimationFrame(() => {
      if (!this.destroyed && this.ctx) {
        this.pivotSvc.render(this.ctx);
      }
    });
  }

  // ── Unified mousedown ────────────────────────────────────────────────────
  private onMouseDown(e: MouseEvent) {
    const { px, py } = this.canvasPos(e);

    // ── Horizontal scrollbar ─────────────────────────────────────────────
    const hthumb = this.pivotSvc.hscrollbar_thumb_rect();
    if (
      hthumb[2] > 0 &&
      px >= hthumb[0] &&
      px <= hthumb[0] + hthumb[2] &&
      py >= hthumb[1] &&
      py <= hthumb[1] + hthumb[3]
    ) {
      this.hScrollDrag = true;
      this.hScrollDragStartX = px;
      this.hScrollDragStartScroll = this.pivotSvc.get_scroll_x();
      e.preventDefault();
      return;
    }

    // ── Vertical scrollbar ───────────────────────────────────────────────
    const thumb = Array.from(this.pivotSvc.scrollbar_thumb_rect());
    if (
      px >= thumb[0] &&
      px <= thumb[0] + thumb[2] &&
      py >= thumb[1] &&
      py <= thumb[1] + thumb[3]
    ) {
      this.scrollDrag = true;
      this.scrollDragStartY = py;
      this.scrollDragStartScroll = this.pivotSvc.get_scroll_y();
      e.preventDefault();
      return;
    }

    // ── Column resize ────────────────────────────────────────────────────
    const borderCol = this.getBorderColAt(px, py);
    if (borderCol >= 0) {
      const visibleWidths = this.pivotSvc.get_visible_col_widths();
      this.drag = {
        active: true,
        colIdx: borderCol,
        startX: px,
        startWidth: visibleWidths[borderCol],
      };
      e.preventDefault();
      return;
    }

    // ── Column swap drag — only in header, not on border ─────────────────
    if (py >= 0 && py <= 54) {
      const colIdx = this.getColAtScreenX(px);
      // don't allow dragging fixed col (idx 0) or -1
      if (colIdx > 0) {
        this.colSwap = {
          active: true,
          fromIdx: colIdx,
          startX: px,
          startY: py,
          currentX: px,
          targetIdx: colIdx,
        };
        e.preventDefault();
      }
    }
  }

  // ── Unified mousemove ────────────────────────────────────────────────────
  private onMouseMove(e: MouseEvent) {
    const canvas = this.canvasRef.nativeElement;
    const { px, py } = this.canvasPos(e);

    if (this.hScrollDrag) {
      const delta = px - this.hScrollDragStartX;
      const totalW = this.pivotSvc
        .get_visible_col_widths()
        .reduce((a: number, b: number) => a + b, 0);
      const viewportW = canvas.width - 10;
      const scrollRatio = totalW / viewportW;
      this.pivotSvc.set_scroll_x(
        this.hScrollDragStartScroll + delta * scrollRatio,
      );
      this.pivotSvc.render(this.ctx!);
      return;
    }

    if (this.scrollDrag) {
      const delta = py - this.scrollDragStartY;
      const contentH = this.pivotSvc.total_content_height();
      const viewportH = canvas.height - 54;
      const scrollRatio = contentH / viewportH;
      this.pivotSvc.set_scroll_y(
        this.scrollDragStartScroll + delta * scrollRatio,
      );
      this.pivotSvc.render(this.ctx!);
      return;
    }

    if (this.drag.active) {
      const delta = px - this.drag.startX;
      const newWidth = Math.max(30, this.drag.startWidth + delta);
      this.pivotSvc.set_visible_col_width(this.drag.colIdx, newWidth);
      this.pivotSvc.render(this.ctx!);
      canvas.style.cursor = "col-resize";
      return;
    }

    if (this.colSwap.active) {
      this.colSwap.currentX = px;
      this.colSwap.targetIdx = this.getColAtScreenX(px);
      canvas.style.cursor = "grabbing";
      // Render with ghost overlay
      this.pivotSvc.render(this.ctx!);
      this.drawSwapGhost();
      return;
    }

    // Hover cursor
    const borderCol = this.getBorderColAt(px, py);
    if (borderCol !== this.hoverColBorder) {
      this.hoverColBorder = borderCol;
      if (borderCol >= 0) {
        canvas.style.cursor = "col-resize";
      } else if (py >= 0 && py <= 54 && this.getColAtScreenX(px) > 0) {
        canvas.style.cursor = "grab";
      } else {
        canvas.style.cursor = "default";
      }
    }
  }

  // ── Unified mouseup ──────────────────────────────────────────────────────
  private onMouseUp(e: MouseEvent) {
    const canvas = this.canvasRef.nativeElement;

    if (this.colSwap.active) {
      const { fromIdx, targetIdx } = this.colSwap;
      this.colSwap.active = false;
      this.colSwap.fromIdx = -1;
      this.colSwap.targetIdx = -1;
      canvas.style.cursor = "default";

      // Perform swap if dropped on a different valid column
      if (targetIdx > 0 && targetIdx !== fromIdx) {
        this.pivotSvc.swap_visible_columns(fromIdx, targetIdx);
      }
      this.pivotSvc.render(this.ctx!);
      return;
    }

    this.hScrollDrag = false;
    this.scrollDrag = false;
    if (this.drag.active) {
      this.drag.active = false;
      this.drag.colIdx = -1;
      canvas.style.cursor = "default";
    }
  }

  private onMouseLeave(e: MouseEvent) {
    const canvas = this.canvasRef.nativeElement;
    this.hScrollDrag = false;
    this.scrollDrag = false;
    this.colSwap.active = false;
    if (this.drag.active) this.drag.active = false;
    this.hoverColBorder = -1;
    canvas.style.cursor = "default";
    this.pivotSvc.render(this.ctx!);
  }

  // ── Click ────────────────────────────────────────────────────────────────
  private onCanvasClick(e: MouseEvent) {
    // Ignore if mouse moved significantly (was a drag)
    if (Math.abs(e.movementX) > 3) return;
    const { px, py } = this.canvasPos(e);

    // Column group toggle
    if (this.pivotSvc.hit_test_col_toggle(px, py, 30.0, 30.0)) {
      this.pivotSvc.toggle_col_group();
      this.pivotSvc.render(this.ctx!);
      return;
    }

    // Row toggle
    const firstColWidth = this.pivotSvc.get_col_widths()[0] ?? 120;
    if (px <= firstColWidth && px <= 24.0) {
      const rowIdx = this.pivotSvc.hit_test_row(
        py,
        this.pivotSvc.get_scroll_y(),
        0.0,
        54.0,
        25.0,
      );
      if (rowIdx >= 0) {
        this.pivotSvc.toggle_row(rowIdx);
        this.pivotSvc.render(this.ctx!);
      }
    }
  }

  // ── Swap ghost overlay ───────────────────────────────────────────────────
  private drawSwapGhost(): void {
    const ctx = this.ctx!;
    const { fromIdx, currentX, targetIdx } = this.colSwap;
    const widths = this.pivotSvc.get_visible_col_widths();
    const scrollX = this.pivotSvc.get_scroll_x();

    // Get screen x of dragged column center
    const fromScreenX = this.getColScreenLeft(fromIdx);
    const fromW = widths[fromIdx] ?? 80;

    // Draw semi-transparent ghost of dragged column
    ctx.save();
    ctx.globalAlpha = 0.45;
    ctx.fillStyle = "#3b82f6";
    ctx.fillRect(currentX - fromW / 2, 0, fromW, 54);
    ctx.globalAlpha = 1.0;

    // Column name label on ghost
    ctx.fillStyle = "#ffffff";
    ctx.font = "bold 13px sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    const colName = this.pivotSvc.get_col_name_at_visible_idx(fromIdx);
    ctx.fillText(colName, currentX, 27, fromW - 8);

    // Drop target highlight
    if (targetIdx > 0 && targetIdx !== fromIdx) {
      const targetLeft = this.getColScreenLeft(targetIdx);
      const targetW = widths[targetIdx] ?? 80;
      ctx.globalAlpha = 0.25;
      ctx.fillStyle = "#22c55e";
      ctx.fillRect(targetLeft, 0, targetW, 54);
      ctx.globalAlpha = 1.0;

      // Drop indicator line
      ctx.strokeStyle = "#16a34a";
      ctx.lineWidth = 3;
      ctx.beginPath();
      // Line on the side closer to drag direction
      const lineX =
        currentX > fromScreenX
          ? targetLeft + targetW // dropping to the right
          : targetLeft; // dropping to the left
      ctx.moveTo(lineX, 0);
      ctx.lineTo(lineX, 54);
      ctx.stroke();
    }

    ctx.restore();
  }

  // ── Helpers ──────────────────────────────────────────────────────────────

  /** Returns visible col index at screen px, or -1 */
  private getColAtScreenX(px: number): number {
    const widths = this.pivotSvc.get_visible_col_widths();
    const scrollX = this.pivotSvc.get_scroll_x();

    // Fixed col (competition)
    if (px < widths[0]) return 0;

    // Scrollable cols
    let x = widths[0] - scrollX;
    for (let i = 1; i < widths.length; i++) {
      if (px >= x && px < x + widths[i]) return i;
      x += widths[i];
    }
    return -1;
  }

  /** Returns screen left edge of a visible col */
  private getColScreenLeft(visibleIdx: number): number {
    const widths = this.pivotSvc.get_visible_col_widths();
    const scrollX = this.pivotSvc.get_scroll_x();
    if (visibleIdx === 0) return 0;
    let x = widths[0] - scrollX;
    for (let i = 1; i < visibleIdx; i++) {
      x += widths[i];
    }
    return x;
  }

  /** Returns col index if px is near a column border in the header, else -1 */
  private getBorderColAt(px: number, py: number): number {
    if (py < 0 || py > 54) return -1;
    const widths = this.pivotSvc.get_visible_col_widths();
    const scrollX = this.pivotSvc.get_scroll_x();

    // Fixed col right border
    if (Math.abs(px - widths[0]) <= this.RESIZE_HIT_ZONE) return 0;

    // Scrollable cols
    let x = widths[0] - scrollX;
    for (let i = 1; i < widths.length; i++) {
      x += widths[i];
      if (x < widths[0]) continue; // behind pinned col
      if (Math.abs(x - px) <= this.RESIZE_HIT_ZONE) return i;
    }
    return -1;
  }

  private canvasPos(e: MouseEvent): { px: number; py: number } {
    const rect = this.canvasRef.nativeElement.getBoundingClientRect();
    return { px: e.clientX - rect.left, py: e.clientY - rect.top };
  }
}
