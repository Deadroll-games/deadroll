// The MagicBlock package (Anchor 0.30, web3.js v1) encodes with the Node global `Buffer`, which a
// browser does not have. magicblock.ts imports this first, so it is set before the package runs.
import { Buffer } from "buffer";

const g = globalThis as { Buffer?: typeof Buffer };
g.Buffer ??= Buffer;
