// A driver that does enough work to be profiled, importing the built bundle so
// the profile's frames are the bundle's own — which is what makes mapping them
// back through the source map a real test rather than a synthetic one.
import { main } from "./dist/index.js";

const values = Array.from({ length: 2000 }, (_, i) => Math.sin(i) * 1000);

async function run() {
  let total = 0;
  for (let round = 0; round < 400; round += 1) {
    const result = await main(values);
    total += result.length;
  }
  return total;
}

run().then((total) => console.log("done", total));
