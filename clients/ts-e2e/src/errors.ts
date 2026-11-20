// Contract error helpers. Self-contained so the frontend and backend can copy
// this file as is: it has no imports and accepts any generated client's
// `AssembledTransaction`, whichever @stellar/stellar-sdk copy built it.

/** The part of an `AssembledTransaction` these helpers read. */
export interface SimulatedTx {
  simulation?: object;
}

/** Numeric `ContractError` codes (src/errors.rs). Codes are never renumbered. */
export const ErrorCode = {
  AlreadyInitialized: 1,
  NotInitialized: 2,
  FolderNameTaken: 3,
  FolderNotFound: 4,
  PortfolioNotFound: 5,
  TemplateNotFound: 6,
  Unauthorized: 7,
  InvalidCID: 8,
  InvalidFolderName: 9,
  AdminAlreadyExists: 10,
  AdminNotFound: 11,
  CannotRemoveLastAdmin: 12,
  PortfolioAlreadyMinted: 13,
  InvalidMetadataURI: 14,
  TemplateAlreadyExists: 15,
  FolderAlreadyClaimed: 16,
  InvalidTemplateID: 17,
  TemplateLimitReached: 18,
  TooManyAdmins: 19,
  UpdateTooSoon: 20,
  InvalidUpdateInterval: 21,
  InvalidBatchSize: 22,
} as const;

const CONTRACT_ERROR = /Error\(Contract, #(\d+)\)/;

/** Extracts the contract error code from an error message, if any. */
export function parseContractErrorCode(message: string): number | undefined {
  const match = CONTRACT_ERROR.exec(message);
  return match ? Number(match[1]) : undefined;
}

/**
 * Returns the contract error code of a simulated (not yet sent) transaction,
 * or undefined if the simulation succeeded.
 */
export function simulationErrorCode(tx: SimulatedTx): number | undefined {
  const error = (tx.simulation as { error?: unknown } | undefined)?.error;
  return typeof error === "string" ? parseContractErrorCode(error) : undefined;
}

/**
 * Awaits a contract call and returns its contract error code. Handles both a
 * failed simulation and a thrown error (e.g. a failed submission). Throws if
 * the call succeeded.
 */
export async function contractErrorCode(
  call: Promise<SimulatedTx>,
): Promise<number> {
  let tx: SimulatedTx;
  try {
    tx = await call;
  } catch (err) {
    const code = parseContractErrorCode(String((err as Error)?.message ?? err));
    if (code === undefined) throw err;
    return code;
  }
  const code = simulationErrorCode(tx);
  if (code === undefined) {
    throw new Error("expected a contract error, but the simulation succeeded");
  }
  return code;
}
