// Deliberately the largest module, and only reachable through a dynamic
// import, so that initial-load bytes and total bytes differ by a lot.
export function analyse(values: number[]): string {
  const sorted = [...values].sort((left, right) => left - right);
  const sum = sorted.reduce((total, value) => total + value, 0);
  const mean = sum / (sorted.length || 1);
  const variance =
    sorted.reduce((total, value) => total + (value - mean) ** 2, 0) /
    (sorted.length || 1);

  const median =
    sorted.length % 2 === 0
      ? (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2
      : sorted[(sorted.length - 1) / 2];

  return [
    `n=${sorted.length}`,
    `mean=${mean.toFixed(4)}`,
    `median=${median?.toFixed(4) ?? "n/a"}`,
    `sd=${Math.sqrt(variance).toFixed(4)}`,
    `min=${sorted[0]?.toFixed(4) ?? "n/a"}`,
    `max=${sorted[sorted.length - 1]?.toFixed(4) ?? "n/a"}`,
  ].join(" ");
}
