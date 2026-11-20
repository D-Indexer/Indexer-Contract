import { describe, expect, it, beforeAll } from "vitest";
import { Keypair, scValToNative, xdr } from "@stellar/stellar-sdk";
import type { Client } from "bindings";
import {
  clientFor,
  deployment,
  fundedAccount,
  server,
  waitForLedger,
} from "../src/env";
import { contractErrorCode, ErrorCode } from "../src/errors";

const CID_1 = "bafkreicu2xn7aesolgx3umlcnw3tozt4xibpu42wa2i34dg3bz6kxgy7y4";
const CID_2 = "QmU3p6RJzZsp4ZTsi6JWeCDxD3JPinVgsZB2uRUS6APEce";
const TEMPLATE_ID = deployment.templates[0]?.template_id ?? 1;

let user: Keypair;
let client: Client;
let folder: string;
let startLedger: number;

beforeAll(async () => {
  user = await fundedAccount();
  client = clientFor(user);
  folder = `e2e${Date.now().toString(36)}.folder`;
  startLedger = (await server.getLatestLedger()).sequence;
});

describe("full flow", () => {
  it("claims a folder", async () => {
    const tx = await client.claim_folder({
      owner: user.publicKey(),
      folder_name: folder,
    });
    const { result } = await tx.signAndSend();
    expect(result.isOk()).toBe(true);

    const owner = (await client.get_folder({ folder_name: folder })).result;
    expect(owner.unwrap()).toBe(user.publicKey());
    const identity = (await client.get_identity({ owner: user.publicKey() }))
      .result;
    expect(identity.unwrap()).toBe(folder);
  });

  it("mints a portfolio", async () => {
    const tx = await client.mint_portfolio({
      owner: user.publicKey(),
      template_id: TEMPLATE_ID,
      ipfs_cid: CID_1,
    });
    await tx.signAndSend();

    const nft = (await client.get_portfolio({ owner: user.publicKey() }))
      .result.unwrap();
    expect(nft.owner).toBe(user.publicKey());
    expect(nft.template_id).toBe(TEMPLATE_ID);
    expect(nft.ipfs_cid).toBe(CID_1);
    expect(typeof nft.minted_at).toBe("bigint");
  });

  it("updates the portfolio once the rate-limit interval has passed", async () => {
    const interval = (await client.get_min_update_interval()).result;
    expect(typeof interval).toBe("number");

    // Straight after the mint the update is rate-limited.
    if (interval > 0) {
      expect(
        await contractErrorCode(
          client.update_portfolio({ owner: user.publicKey(), ipfs_cid: CID_2 }),
        ),
      ).toBe(ErrorCode.UpdateTooSoon);
    }

    const latest = (await server.getLatestLedger()).sequence;
    await waitForLedger(latest + interval + 1);
    await (
      await client.update_portfolio({ owner: user.publicKey(), ipfs_cid: CID_2 })
    ).signAndSend();

    const nft = (await client.get_portfolio({ owner: user.publicKey() }))
      .result.unwrap();
    expect(nft.ipfs_cid).toBe(CID_2);
  });

  it("verifies portfolios", async () => {
    expect(
      (await client.verify_portfolio({ owner: user.publicKey() })).result.unwrap(),
    ).toBe(true);
    const stranger = Keypair.random().publicKey();
    expect(
      (await client.verify_portfolio({ owner: stranger })).result.unwrap(),
    ).toBe(false);
  });

  it("retrieves the flow's events with getEvents", async () => {
    const { events } = await server.getEvents({
      startLedger,
      filters: [{ type: "contract", contractIds: [deployment.contract_id] }],
      limit: 200,
    });
    const decoded = events.map((e) => ({
      topic: scValToNative(e.topic[0] as xdr.ScVal) as string,
      data: scValToNative(e.value) as unknown[],
    }));
    const mine = decoded.filter((e) => e.data[0] === user.publicKey());

    expect(mine.map((e) => e.topic)).toEqual([
      "folder_cl",
      "port_mint",
      "port_upd",
    ]);
    expect(mine[0].data).toEqual([user.publicKey(), folder]);
    expect(mine[1].data).toEqual([user.publicKey(), TEMPLATE_ID, CID_1]);
    expect(mine[2].data).toEqual([user.publicKey(), CID_2]);
  });
});

describe("error decoding by numeric code", () => {
  let other: Keypair;
  let otherClient: Client;

  beforeAll(async () => {
    other = await fundedAccount();
    otherClient = clientFor(other);
  });

  it("FolderNameTaken (3)", async () => {
    expect(
      await contractErrorCode(
        otherClient.claim_folder({ owner: other.publicKey(), folder_name: folder }),
      ),
    ).toBe(ErrorCode.FolderNameTaken);
  });

  it("FolderNotFound (4)", async () => {
    expect(
      await contractErrorCode(
        otherClient.mint_portfolio({
          owner: other.publicKey(),
          template_id: TEMPLATE_ID,
          ipfs_cid: CID_1,
        }),
      ),
    ).toBe(ErrorCode.FolderNotFound);
  });

  it("PortfolioNotFound (5)", async () => {
    expect(
      await contractErrorCode(
        otherClient.update_portfolio({ owner: other.publicKey(), ipfs_cid: CID_1 }),
      ),
    ).toBe(ErrorCode.PortfolioNotFound);
  });

  it("TemplateNotFound (6)", async () => {
    expect(
      await contractErrorCode(otherClient.get_template({ template_id: 9999 })),
    ).toBe(ErrorCode.TemplateNotFound);
  });

  it("Unauthorized (7)", async () => {
    expect(
      await contractErrorCode(
        otherClient.register_template({
          admin: other.publicKey(),
          template_id: 9999,
          metadata_uri: `ipfs://${CID_1}`,
        }),
      ),
    ).toBe(ErrorCode.Unauthorized);
  });

  it("InvalidCID (8)", async () => {
    expect(
      await contractErrorCode(
        client.update_portfolio({ owner: user.publicKey(), ipfs_cid: "not-a-cid" }),
      ),
    ).toBe(ErrorCode.InvalidCID);
  });

  it("InvalidFolderName (9)", async () => {
    expect(
      await contractErrorCode(
        otherClient.claim_folder({ owner: other.publicKey(), folder_name: "x.folder" }),
      ),
    ).toBe(ErrorCode.InvalidFolderName);
  });

  it("PortfolioAlreadyMinted (13)", async () => {
    expect(
      await contractErrorCode(
        client.mint_portfolio({
          owner: user.publicKey(),
          template_id: TEMPLATE_ID,
          ipfs_cid: CID_1,
        }),
      ),
    ).toBe(ErrorCode.PortfolioAlreadyMinted);
  });
});
