// The entry point. Imports one module eagerly and one lazily, so a split build
// has something real to put on the critical path and something to leave off it.
import { format } from "./format";
import { CONSTANTS } from "./constants";

export async function main(values: number[]): Promise<string> {
  const summary = format(values);

  // Dynamic import: this lands in its own chunk under --splitting, which is
  // what makes "initial load" a different number from "everything".
  const { analyse } = await import("./analysis");

  return `${summary} ${analyse(values)} ${CONSTANTS.version}`;
}
