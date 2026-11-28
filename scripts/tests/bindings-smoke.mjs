// Smoke test for the generated TypeScript bindings: import the built package
// and construct a client against the default (testnet) network.
// Usage: node scripts/tests/bindings-smoke.mjs [bindings/ts]
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { readFileSync } from "node:fs";

const dir = resolve(process.argv[2] ?? "bindings/ts");
const pkg = JSON.parse(readFileSync(`${dir}/package.json`, "utf8"));
const entry = pkg.exports?.["."]?.import ?? pkg.exports?.["."]?.default ?? pkg.main ?? "dist/index.js";
const mod = await import(pathToFileURL(resolve(dir, entry)).href);

const net = mod.networks?.testnet;
if (!net?.contractId) throw new Error("networks.testnet.contractId missing from bindings");

const client = new mod.Client({
  ...net,
  rpcUrl: "https://soroban-testnet.stellar.org",
});
if (client.options.contractId !== net.contractId) throw new Error("client contractId mismatch");
console.log(`bindings ok: Client constructed for ${net.contractId}`);
