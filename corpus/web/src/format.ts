// Small, eagerly imported, and on the critical path.
export function format(values: number[]): string {
  return values.map((value) => value.toFixed(2)).join(", ");
}

export function pad(text: string, width: number): string {
  return text.length >= width ? text : text.padStart(width, " ");
}
