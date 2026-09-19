import type { EvidenceKind } from "./models";

export type EvidenceDisplayKind = "static" | "observed" | "inferred";

export function evidenceDisplayKind(kind: EvidenceKind): EvidenceDisplayKind {
  if (kind === "observed") return "observed";
  if (kind === "inferred") return "inferred";
  return "static";
}

export function evidenceLabel(kind: EvidenceKind): string {
  if (kind === "declared") return "Static · declared";
  if (kind === "resolved") return "Static · resolved";
  if (kind === "observed") return "Observed";
  return "Inferred";
}
