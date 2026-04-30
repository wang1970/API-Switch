import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Format a response_ms value for display.
 * Handles both new format (raw ms number like "1234") and legacy format ("1.2s" / "350ms").
 * Returns "X" for error markers, empty string for empty input.
 */
export function formatResponseMs(value: string | null | undefined): string {
  if (!value) return "";
  if (value === "X") return "X";

  // New format: raw ms number
  const ms = parseInt(value, 10);
  if (!isNaN(ms)) {
    if (ms >= 1000) {
      return `${(ms / 1000).toFixed(1)}s`;
    }
    return `${ms}ms`;
  }

  // Legacy format: "1.2s" / "350ms" — pass through as-is
  return value;
}
