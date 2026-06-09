// import { Injectable, signal, computed } from "@angular/core";
// import { WasmService } from "./wasm.service";
// import {
//   DataRow,
//   PivotConfig,
//   PivotResult,
//   RenderConfig,
//   DEFAULT_RENDER_CONFIG,
// } from "../models/pivot.models";

// @Injectable({ providedIn: "root" })
// export class PivotService {
//   private engine: any = null;

//   // Signals
//   data = signal<DataRow[]>([]);
//   config = signal<PivotConfig>({
//     rows: [],
//     columns: [],
//     values: [],
//     filters: [],
//   });
//   result = signal<PivotResult | null>(null);
//   renderConfig = signal<RenderConfig>({ ...DEFAULT_RENDER_CONFIG });
//   availableFields = signal<string[]>([]);
//   isComputing = signal(false);
//   error = signal<string | null>(null);

//   constructor(private wasmService: WasmService) {}

//   setData(rows: DataRow[]): void {
//     this.data.set(rows);
//     const fields = rows.length > 0 ? Object.keys(rows[0].fields) : [];
//     this.availableFields.set(fields);

//     if (this.engine) {
//       this.engine.update_data(rows);
//     }
//   }

//   async initEngine(): Promise<void> {
//     const d = this.data();
//     const c = this.config();
//     this.engine = this.wasmService.createEngine(d, c);
//   }

//   async compute(): Promise<void> {
//     this.isComputing.set(true);
//     this.error.set(null);
//     try {
//       if (!this.engine) await this.initEngine();
//       else this.engine.update_config(this.config());

//       const result: PivotResult = this.engine.compute();
//       this.result.set(result);
//     } catch (e: any) {
//       this.error.set(e.toString());
//     } finally {
//       this.isComputing.set(false);
//     }
//   }

//   render(
//     ctx: CanvasRenderingContext2D,
//     scrollX = 0,
//     scrollY = 0,
//     highlightRow = -1,
//     highlightCol = -1,
//   ): void {
//     if (!this.engine || !this.result()) return;
//     this.engine.render(
//       ctx,
//       this.renderConfig(),
//       scrollX,
//       scrollY,
//       highlightRow,
//       highlightCol,
//     );
//   }

//   hitTest(px: number, py: number, scrollX: number, scrollY: number) {
//     if (!this.engine) return null;
//     const cfg = this.renderConfig();
//     return this.engine.hit_test(
//       px,
//       py,
//       cfg.cell_width,
//       cfg.cell_height,
//       scrollX,
//       scrollY,
//     );
//   }

//   exportCsv(): void {
//     if (!this.engine) return;
//     const csv = this.engine.export_csv();
//     const blob = new Blob([csv], { type: "text/csv" });
//     const url = URL.createObjectURL(blob);
//     const a = document.createElement("a");
//     a.href = url;
//     a.download = "pivot-export.csv";
//     a.click();
//     URL.revokeObjectURL(url);
//   }

//   updateRenderConfig(partial: Partial<RenderConfig>): void {
//     this.renderConfig.update((c) => ({ ...c, ...partial }));
//   }

//   updateConfig(partial: Partial<PivotConfig>): void {
//     this.config.update((c) => ({ ...c, ...partial }));
//   }
// }
