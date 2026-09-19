export const MAX_FAILURE_MESSAGE_CHARACTERS = 480;

function normalizedCandidate(value: unknown): string | null {
  let text: string;
  try {
    if (typeof value === "string") text = value;
    else if (value instanceof Error) text = value.message;
    else return null;
  } catch {
    return null;
  }

  const normalized = text.replace(/\s+/gu, " ").trim();
  if (!normalized) return null;
  const characters = Array.from(normalized);
  if (characters.length <= MAX_FAILURE_MESSAGE_CHARACTERS) return normalized;
  return `${characters.slice(0, MAX_FAILURE_MESSAGE_CHARACTERS - 1).join("")}…`;
}

/** Converts native IPC string rejections and JavaScript errors into safe UI copy. */
export function failureMessage(failure: unknown, fallback: string): string {
  return normalizedCandidate(failure) ?? normalizedCandidate(fallback) ?? "Unexpected failure";
}
