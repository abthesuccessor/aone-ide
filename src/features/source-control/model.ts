export interface GitFileStatus {
  relativePath: string;
  indexStatus: string;
  workingTreeStatus: string;
  staged: boolean;
  conflicted: boolean;
}

export interface GitStatusResult {
  isRepository: boolean;
  branch?: string;
  head?: string;
  files: GitFileStatus[];
}

export interface GitDiffResult {
  relativePath: string;
  staged: boolean;
  content: string;
  truncated: boolean;
  originalContent?: string;
  modifiedContent?: string;
  comparisonTruncated: boolean;
}

export interface GitMutationResult {
  ok: boolean;
  summary: string;
}
