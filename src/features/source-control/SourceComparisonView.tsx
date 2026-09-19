import { DiffEditor, type BeforeMount } from "@monaco-editor/react";
import { GitDiff, X } from "@phosphor-icons/react";
import { useCallback } from "react";
import type { WorkbenchSettings } from "../editor/model";
import { monacoThemeName, registerMonacoThemes } from "../editor/themes";
import { monacoLanguageId } from "../../components/SourceView";
import type { GitDiffResult } from "./model";

interface SourceComparisonViewProps {
  diff: GitDiffResult;
  language: string;
  settings: WorkbenchSettings;
  onClose: () => void;
}

export function SourceComparisonView({ diff, language, settings, onClose }: SourceComparisonViewProps) {
  const beforeMount: BeforeMount = useCallback((monaco) => registerMonacoThemes(monaco), []);
  const comparisonAvailable = diff.originalContent !== undefined
    && diff.modifiedContent !== undefined
    && !diff.comparisonTruncated;
  return (
    <section className="source-comparison" aria-label={`Git comparison for ${diff.relativePath}`}>
      <header className="source-header">
        <GitDiff size={14} weight="duotone" aria-hidden="true" />
        <span className="source-path">{diff.relativePath}</span>
        <span className="source-range">{diff.staged ? "HEAD to staged" : "Previous to working tree"}</span>
        <div className="source-actions">
          <button type="button" onClick={onClose} title="Close comparison"><X size={13} />Close diff</button>
        </div>
      </header>
      <div className="source-editor">
        {comparisonAvailable ? (
          <DiffEditor
            original={diff.originalContent}
            modified={diff.modifiedContent}
            language={monacoLanguageId(language)}
            theme={monacoThemeName(settings.theme)}
            beforeMount={beforeMount}
            loading={<div className="editor-loading">Loading Git comparison...</div>}
            options={{
              readOnly: true,
              originalEditable: false,
              automaticLayout: true,
              renderSideBySide: true,
              enableSplitViewResizing: true,
              minimap: { enabled: false },
              fontFamily: settings.fontFamily,
              fontSize: settings.fontSize,
              lineHeight: settings.lineHeight,
              wordWrap: settings.wordWrap,
              wordWrapColumn: settings.wordWrapColumn,
              scrollBeyondLastLine: false,
              overviewRulerLanes: 0,
              overviewRulerBorder: false,
            }}
          />
        ) : (
          <pre className="source-comparison-patch">{diff.content || "No textual comparison is available for this file."}</pre>
        )}
      </div>
    </section>
  );
}
