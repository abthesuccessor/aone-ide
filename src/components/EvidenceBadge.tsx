import { Eye, Sparkle, TreeStructure } from "@phosphor-icons/react";
import { evidenceDisplayKind, evidenceLabel, type EvidenceKind } from "../types";

interface EvidenceBadgeProps {
  evidence: EvidenceKind;
  compact?: boolean;
}

export function EvidenceBadge({ evidence, compact = false }: EvidenceBadgeProps) {
  const display = evidenceDisplayKind(evidence);
  const Icon = display === "observed" ? Eye : display === "inferred" ? Sparkle : TreeStructure;
  return (
    <span className={`evidence-badge evidence-${display}`} title={evidenceLabel(evidence)}>
      <Icon size={compact ? 10 : 12} weight="bold" aria-hidden="true" />
      {!compact && <span>{evidenceLabel(evidence)}</span>}
    </span>
  );
}
