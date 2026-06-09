import { Component } from "@angular/core";
import { PivotTableComponent } from "./pivot-table.component";

@Component({
  selector: "app-root",
  standalone: true,
  imports: [PivotTableComponent],
  template: `<app-pivot-table />`,
  styles: [
    `
      :host {
        display: block;
        height: 100vh;
        margin: 0;
      }
      * {
        box-sizing: border-box;
      }
    `,
  ],
})
export class AppComponent {}
