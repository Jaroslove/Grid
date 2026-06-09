import { Injectable } from "@angular/core";
import { BehaviorSubject } from "rxjs";

@Injectable({ providedIn: "root" })
export class WasmService {
  private wasmModule: any = null;
  private ready$ = new BehaviorSubject<boolean>(false);
  readonly isReady$ = this.ready$.asObservable();

  async load(): Promise<void> {
    // Wait for the script in index.html to finish
    await new Promise<void>(resolve => {
      const check = setInterval(() => {
        if ((window as any).__pivotWasm) {
          clearInterval(check);
          resolve();
        }
      }, 50);
    });
    this.wasmModule = (window as any).__pivotWasm;
    this.ready$.next(true);
    console.log("✅ WASM module loaded");
  }

  get module(): any {
    return this.wasmModule;
  }

  createEngine(data: any[]): any {
    if (!this.wasmModule) throw new Error("WASM not loaded");
    return new this.wasmModule.PivotEngine(data);
  }
}
