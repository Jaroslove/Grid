// import { DataRow } from "../models/pivot.models";

// export const SAMPLE_DATA: DataRow[] = generateSampleData();

// function generateSampleData(): DataRow[] {
//   const regions = ["North", "South", "East", "West"];
//   const categories = ["Electronics", "Clothing", "Food", "Furniture"];
//   const quarters = ["Q1", "Q2", "Q3", "Q4"];
//   const salesReps = ["Alice", "Bob", "Carol", "Dave", "Eve"];

//   const rows: DataRow[] = [];

//   for (const region of regions) {
//     for (const category of categories) {
//       for (const quarter of quarters) {
//         for (const rep of salesReps) {
//           const sales = Math.floor(Math.random() * 50000 + 5000);
//           const units = Math.floor(Math.random() * 200 + 10);
//           const profit = Math.floor(sales * (Math.random() * 0.3 + 0.1));

//           rows.push({
//             fields: {
//               Region: region,
//               Category: category,
//               Quarter: quarter,
//               SalesRep: rep,
//               Sales: sales.toString(),
//               Units: units.toString(),
//               Profit: profit.toString(),
//             },
//           });
//         }
//       }
//     }
//   }

//   return rows;
// }
