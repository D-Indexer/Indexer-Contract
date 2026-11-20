# TypeScript client e2e suite

Drives a deployed contract through the generated TypeScript bindings on a local
quickstart network: claim, mint, update and verify; contract error decoding by
numeric code; and event retrieval through `getEvents`.

```bash
docker run -d -p 8000:8000 stellar/quickstart:latest --local --enable core,rpc,horizon,friendbot
./scripts/deploy.sh local          # from the repo root; writes deployments/local.json
cd clients/ts-e2e
npm run bindings                   # generate + build bindings from target/stellar/*.wasm
npm ci && npm test
```

`DEPLOYMENT` (path relative to the repo root) and `FRIENDBOT_URL` override the defaults.

[`src/errors.ts`](src/errors.ts) has no dependencies: frontend and backend can copy it to map
failed calls to `ContractError` codes (`contractErrorCode(client.some_call(...))`).
