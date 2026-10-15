import type { AppErrorPayload } from "../types";

function isAppError(value: unknown): value is AppErrorPayload {
  return typeof value === "object" && value !== null && "message" in value && typeof value.message === "string";
}

/** Readable message for errors thrown by Tauri commands or the frontend. */
export function errorMessage(error: unknown): string {
  if (isAppError(error)) return error.message;
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "Something went wrong.";
}

/** Whether the backend refused the request as invalid, so retrying cannot help. */
export function isValidationError(error: unknown): boolean {
  return isAppError(error) && error.kind === "validation";
}
