/**
 * Centralized error formatting utility for IPC, network, and validation errors.
 */
export function formatErrorMessage(
  error: unknown,
  fallback = "The action could not be completed.",
): string {
  if (!error) return fallback;
  if (typeof error === "string") {
    const trimmed = error.trim();
    return trimmed || fallback;
  }
  if (error instanceof Error) {
    const trimmed = error.message.trim();
    return trimmed || fallback;
  }
  if (
    typeof error === "object" &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  ) {
    const trimmed = ((error as { message: string }).message || "").trim();
    return trimmed || fallback;
  }
  return String(error);
}
