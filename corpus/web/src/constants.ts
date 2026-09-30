// A block of highly repetitive string data. It exists to make compression
// locality observable: it compresses almost for free, so a change that
// deduplicates it removes raw bytes and can *increase* the gzipped total,
// which is the counterintuitive result TOOLING-WEB §4.1 asks for.
const REPEATED = "binmap-corpus-web-repeated-token-";

export const CONSTANTS = {
  version: "0.1.0",
  tokens: Array.from({ length: 256 }, (_, index) => `${REPEATED}${index}`),
  labels: Array.from({ length: 128 }, (_, index) => `${REPEATED}label-${index}`),
};
