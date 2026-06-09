import {
  Component,
  OnInit,
  OnDestroy,
  ViewChild,
  ElementRef,
  AfterViewInit,
  ChangeDetectionStrategy,
  signal,
  effect,
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

    // ── Resize state ────────────────────────────────────────────────────────
  private drag = {
    active: false,
    colIdx: -1,
    startX: 0,
    startWidth: 0,
  };
  private hoverColBorder = -1;
  private readonly RESIZE_HIT_ZONE = 5;   // px either side of border

  private ctx: CanvasRenderingContext2D | null = null;
  private animFrame: number | null = null;
  private destroyed = false;

  constructor(
    public pivotSvc: PivotService,
    private wasmSvc: WasmService,
    private http: HttpClient,
  ) {
  }

  async ngOnInit(): Promise<void> {
    await this.wasmSvc.load();
    const parsedData = await this.http.get<any>('/assets/winners.json').toPromise();
    const dataRows = populateDataRows(parsedData);
    const increased = Array.from({ length: 10 }, () => dataRows).flat();
    this.pivotSvc.setData(increased);

    await this.pivotSvc.initEngine();
    this.init();
    if (!this.ctx) return;  
    this.pivotSvc.render(
      this.ctx
    );
  }

  ngOnDestroy(): void {
    this.destroyed = true;
    if (this.animFrame) cancelAnimationFrame(this.animFrame);
  }

  init() {
        const canvas = this.canvasRef.nativeElement;
    this.ctx = canvas.getContext('2d')!;

    // Tell engine the fixed canvas size
    this.pivotSvc.set_viewport(canvas.width, canvas.height);
    this.scheduleRender();
    // ── Wheel scroll ──────────────────────────────────────────────────────
    canvas.addEventListener('wheel', (e: WheelEvent) => {
      e.preventDefault();
      if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
        this.pivotSvc.set_scroll_x(this.pivotSvc.get_scroll_x() + (e.deltaX || e.deltaY));
      } else {
        this.pivotSvc.set_scroll_y(this.pivotSvc.get_scroll_y() + e.deltaY);
      }
      this.pivotSvc.render(this.ctx!);
    }, { passive: false });

    // ── Scrollbar drag ────────────────────────────────────────────────────
    let scrollDrag = false;
    let scrollDragStartY = 0;
    let scrollDragStartScroll = 0;
    // ── Horizontal scrollbar drag ─────────────────────────────────────────
    let hScrollDrag = false;
    let hScrollDragStartX = 0;
    let hScrollDragStartScroll = 0;

    canvas.addEventListener('mousedown', (e: MouseEvent) => {
      const { px, py } = this.canvasPos(e);
      const hthumb = this.pivotSvc.hscrollbar_thumb_rect();

      if (hthumb[2] > 0 &&
          px >= hthumb[0] && px <= hthumb[0] + hthumb[2] &&
          py >= hthumb[1] && py <= hthumb[1] + hthumb[3]) {
        hScrollDrag = true;
        hScrollDragStartX = px;
        hScrollDragStartScroll = this.pivotSvc.get_scroll_x();
        e.preventDefault();
        return;
      }
      
      const thumb = Array.from(this.pivotSvc.scrollbar_thumb_rect());

      if (px >= thumb[0] && px <= thumb[0] + thumb[2] &&
          py >= thumb[1] && py <= thumb[1] + thumb[3]) {
        scrollDrag = true;
        scrollDragStartY = py;
        scrollDragStartScroll = this.pivotSvc.get_scroll_y();
        e.preventDefault();
        return;
      }
      this.onMouseDown(e); // existing column resize logic
    });

    canvas.addEventListener('mousemove', (e: MouseEvent) => {
      if (hScrollDrag) {
        const { px } = this.canvasPos(e);
        const delta = px - hScrollDragStartX;
        const totalW = this.pivotSvc.get_visible_col_widths()
                          .reduce((a: number, b: number) => a + b, 0);
        const viewportW = canvas.width - 10;
        const scrollRatio = totalW / viewportW;
        this.pivotSvc.set_scroll_x(hScrollDragStartScroll + delta * scrollRatio);
        this.pivotSvc.render(this.ctx!);
        return;
      }
      if (scrollDrag) {
        const { py } = this.canvasPos(e);
        const delta = py - scrollDragStartY;
        const contentH = this.pivotSvc.total_content_height();
        // In your scroll/hit-test calls, update header offset:
        const viewportH = canvas.height - 54; // two header rows // start_y + header_h
        const scrollRatio = contentH / viewportH;
        this.pivotSvc.set_scroll_y(scrollDragStartScroll + delta * scrollRatio);
        this.pivotSvc.render(this.ctx!);
        return;
      }
      this.onMouseMove(e); // existing resize logic
    });

    canvas.addEventListener('mouseup', (e: MouseEvent) => {
      hScrollDrag = false;  // ← was missing
      scrollDrag = false;
      this.onMouseUp(e);
    });

    canvas.addEventListener('mouseleave', (e: MouseEvent) => {
      hScrollDrag = false;  // ← also reset on leave
      scrollDrag = false;
      this.onMouseLeave(e);
    });

    // ── Row toggle click ──────────────────────────────────────────────────
    canvas.addEventListener('click', (e: MouseEvent) => {
      if (Math.abs(e.movementX) > 2) return;
      const { px, py } = this.canvasPos(e);

      // ── Column group toggle ──────────────────────────────────────────────
      if (this.pivotSvc.hit_test_col_toggle(px, py, 30.0, 30.0)) {
        this.pivotSvc.toggle_col_group();
        this.pivotSvc.render(this.ctx!);
        return;
      }

      // ── Row toggle ───────────────────────────────────────────────────────
      const firstColWidth = this.pivotSvc.get_col_widths()[0] ?? 120;
      if (px <= firstColWidth) {
        // Check click is within the button area (first 24px of the cell)
        const btnAreaWidth = 24.0;
        if (px <= btnAreaWidth) {
          const rowIdx = this.pivotSvc.hit_test_row(
            py, this.pivotSvc.get_scroll_y(),
            0.0,   // start_y
            54.0,  // total header height
            25.0   // row_h
          );
          if (rowIdx >= 0) {
            this.pivotSvc.toggle_row(rowIdx);
            this.pivotSvc.render(this.ctx!);
          }
        }
      }
    });
  }

  scheduleRender(): void {
    if (this.animFrame) cancelAnimationFrame(this.animFrame);
    this.animFrame = requestAnimationFrame(() => {
      if (!this.destroyed && this.ctx) {
        this.pivotSvc.render(
          this.ctx
        );
      }
    });
  }

  /** Returns col index (0-based) if px is within RESIZE_HIT_ZONE of its right border, else -1 */
  private getBorderColAt(px: number, py: number): number {
    if (py < 0 || py > 54) return -1;

    const widths = this.pivotSvc.get_visible_col_widths();
    const scrollX = this.pivotSvc.get_scroll_x();

    // col 0 (competition) is pinned — its right border is at widths[0]
    if (Math.abs(px - widths[0]) <= this.RESIZE_HIT_ZONE) {
      return 0;
    }

    // scrollable columns start after pinned col, offset by scrollX
    let x = widths[0] - scrollX; // virtual x of start of col 1
    for (let i = 1; i < widths.length; i++) {
      x += widths[i];
      const borderScreenX = x; // screen position of this col's right border
      if (borderScreenX < widths[0]) {
        // border is hidden behind pinned col — not resizable
        continue;
      }
      if (Math.abs(borderScreenX - px) <= this.RESIZE_HIT_ZONE) {
        return i;
      }
    }
    return -1;
  }

  // ── Mouse events ─────────────────────────────────────────────────────────
  private onMouseDown(e: MouseEvent) {
    const { px, py } = this.canvasPos(e);
    const col = this.getBorderColAt(px, py);
    if (col >= 0) {
      const visibleWidths = this.pivotSvc.get_visible_col_widths();
      this.drag = {
        active: true,
        colIdx: col,
        startX: px,
        startWidth: visibleWidths[col],
      };
      e.preventDefault();
    }
  }

  private onMouseMove(e: MouseEvent) {
    const canvas = this.canvasRef.nativeElement;
    const { px, py } = this.canvasPos(e);

    if (this.drag.active) {
      const delta = px - this.drag.startX;
      const newWidth = Math.max(30, this.drag.startWidth + delta);
      // map visible index to actual col index via wasm
      this.pivotSvc.set_visible_col_width(this.drag.colIdx, newWidth);
      this.pivotSvc.render(this.ctx!);
      canvas.style.cursor = 'col-resize';
    } else {
      const col = this.getBorderColAt(px, py);
      if (col !== this.hoverColBorder) {
        this.hoverColBorder = col;
        canvas.style.cursor = col >= 0 ? 'col-resize' : 'default';
      }
    }
  }

  private onMouseUp(e: MouseEvent) {
    if (this.drag.active) {
      this.drag.active = false;
      this.drag.colIdx = -1;
      this.canvasRef.nativeElement.style.cursor = 'default';
    }
  }

  private onMouseLeave(e: MouseEvent) {
    if (this.drag.active) {
      this.drag.active = false;
    }
    this.hoverColBorder = -1;
    this.canvasRef.nativeElement.style.cursor = 'default';
  }

  private canvasPos(e: MouseEvent): { px: number; py: number } {
    const rect = this.canvasRef.nativeElement.getBoundingClientRect();
    return {
      px: e.clientX - rect.left,
      py: e.clientY - rect.top,
    };
  }
}
