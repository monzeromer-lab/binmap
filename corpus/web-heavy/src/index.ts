// Every import here is a mistake somebody makes, and each makes one of
// TOOLING-WEB §6's rules fire.

// A whole-library import: one function, the entire package. The fix is
// `lodash/debounce`, and the tool should say so.
import { debounce } from "lodash";

// Polyfills for browsers the project may not actually need to support. The
// number attached to dropping them is a business decision, not a build one.
import "core-js/es/promise";
import "core-js/es/array";

// A Node shim in a browser bundle. Nobody asks for this; it arrives because
// something assumed it was running on a server.
import { Buffer } from "buffer";

export const encode = (value: string): string =>
  Buffer.from(value, "utf8").toString("base64");

export const save = debounce((value: string) => {
  globalThis.localStorage?.setItem("value", encode(value));
}, 250);
