import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function parseResponseMs(value: string | null | undefined): number | null {
  if (!value || value === "X") return null;
  const trimmed = value.trim().toLowerCase();
  const seconds = trimmed.match(/^(\d+(?:\.\d+)?)s$/);
  if (seconds) return Math.round(Number(seconds[1]) * 1000);
  const milliseconds = trimmed.match(/^(\d+(?:\.\d+)?)ms$/);
  if (milliseconds) return Math.round(Number(milliseconds[1]));
  const raw = Number(trimmed);
  return Number.isFinite(raw) ? Math.round(raw) : null;
}

function formatSeconds(ms: number): string {
  const seconds = ms / 1000;
  if (seconds >= 10) return `${seconds.toFixed(1)}s`;
  return `${seconds.toFixed(2).replace(/0+$/, "").replace(/\.$/, "")}s`;
}

/**
 * Format a response_ms value for display.
 * Handles both raw ms numbers ("1234") and legacy values ("1.2s" / "350ms").
 * Always displays seconds; returns "X" for error markers, empty string for empty input.
 */
export function formatResponseMs(value: string | null | undefined): string {
  if (!value) return "";
  if (value === "X") return "X";

  const ms = parseResponseMs(value);
  if (ms !== null) return formatSeconds(ms);

  return value;
}
