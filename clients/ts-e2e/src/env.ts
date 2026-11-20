// Local-network test environment: reads the deployment written by
// `scripts/deploy.sh local` and creates funded, signing clients.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { Keypair, rpc } from "@stellar/stellar-sdk";
import { basicNodeSigner } from "@stellar/stellar-sdk/contract";
import { Client } from "bindings";

const deploymentPath = resolve(
  import.meta.dirname,
  "../../..",
  process.env.DEPLOYMENT ?? "deployments/local.json",
);
export const deployment = JSON.parse(readFileSync(deploymentPath, "utf8")) as {
  contract_id: string;
  rpc_url: string;
  network_passphrase: string;
  admin: string;
  templates: { template_id: number; metadata_uri: string }[];
};

const friendbotUrl =
  process.env.FRIENDBOT_URL ?? "http://localhost:8000/friendbot";

export const server = new rpc.Server(deployment.rpc_url, { allowHttp: true });

export function clientFor(keypair: Keypair): Client {
  return new Client({
    contractId: deployment.contract_id,
    networkPassphrase: deployment.network_passphrase,
    rpcUrl: deployment.rpc_url,
    allowHttp: true,
    publicKey: keypair.publicKey(),
    ...basicNodeSigner(keypair, deployment.network_passphrase),
  });
}

/** A new account funded by the local Friendbot. */
export async function fundedAccount(): Promise<Keypair> {
  const keypair = Keypair.random();
  const res = await fetch(`${friendbotUrl}?addr=${keypair.publicKey()}`);
  if (!res.ok) throw new Error(`friendbot failed: ${res.status}`);
  return keypair;
}

/** Waits until the network has closed `sequence`. */
export async function waitForLedger(sequence: number): Promise<void> {
  while ((await server.getLatestLedger()).sequence < sequence) {
    await new Promise((r) => setTimeout(r, 500));
  }
}
