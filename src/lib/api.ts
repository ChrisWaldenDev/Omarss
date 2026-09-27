// Typed access to the backend. Command signatures and types are generated from Rust into
// `types.ts` (SPEC §4.3); this module only turns their `Result` values into thrown errors.

import { commands, type AppError, type ErrorKind } from "./types";

export class ApiError extends Error {
  readonly kind: ErrorKind;

  constructor(error: AppError) {
    super(error.message);
    this.name = "ApiError";
    this.kind = error.kind;
  }
}

type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

type Commands = typeof commands;

/** Every generated command, returning its data directly and throwing `ApiError` on failure. */
export type Api = {
  [K in keyof Commands]: Commands[K] extends (...args: infer A) => Promise<CommandResult<infer T>>
    ? (...args: A) => Promise<T>
    : never;
};

async function unwrap<T>(pending: Promise<CommandResult<T>>): Promise<T> {
  const result = await pending;
  if (result.status === "error") throw new ApiError(result.error);
  return result.data;
}

export const api = Object.fromEntries(
  Object.entries(commands).map(([name, command]) => [
    name,
    (...args: unknown[]) => unwrap((command as (...a: unknown[]) => Promise<never>)(...args)),
  ]),
) as Api;

/** A user-readable message for anything thrown by `api` or elsewhere. */
export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
