import readline from "node:readline";

const EVENT_PREFIX = "AONE_DEBUG_V1 ";
const CONTROL_PREFIX = "AONE_DEBUG_CONTROL_V1 ";
const enabled = process.env.AONE_DEBUG_PROTOCOL === "AONE_DEBUG_V1";
const nonce = process.env.AONE_DEBUG_NONCE ?? "";
const debugSessionId = process.env.AONE_DEBUG_SESSION_ID ?? "";

let sequence = 0;
let controlEpoch = 0;
let workflowNumber = 0;
let activeWorkflowId = null;
let mode = "running";
let pendingPause = null;
let pausedResolver = null;
let workflowTail = Promise.resolve();
let lastStepId = null;
let workflowTerminalEmitted = false;

if (enabled) {
  if (!nonce || !debugSessionId) {
    throw new Error("AONE_DEBUG_NONCE and AONE_DEBUG_SESSION_ID are required in debug mode");
  }
  const controls = readline.createInterface({ input: process.stdin, terminal: false });
  controls.on("line", receiveControl);
}

export async function withDebugWorkflow(operation) {
  if (!enabled) return operation(null);

  let release;
  const turn = workflowTail;
  workflowTail = new Promise((resolve) => {
    release = resolve;
  });
  await turn;
  workflowNumber += 1;
  const workflowId = `request:${workflowNumber}`;
  activeWorkflowId = workflowId;
  mode = "initialPause";
  lastStepId = null;
  workflowTerminalEmitted = false;
  try {
    const result = await operation(workflowId);
    if (!workflowTerminalEmitted) {
      await emitAdapterTerminal(workflowId, "workflowCompleted");
    }
    return result;
  } catch (error) {
    if (!workflowTerminalEmitted) {
      await emitAdapterTerminal(workflowId, "workflowFailed");
    }
    throw error;
  } finally {
    activeWorkflowId = null;
    pendingPause = null;
    mode = "running";
    release();
  }
}

export async function debugSafePoint({
  workflowId,
  stepId,
  parentStepId,
  kind,
  flowStage,
  source,
  operation,
  resource,
  branchOutcome,
  dataPreview,
  terminal,
}) {
  if (!enabled || !workflowId) return;
  if (workflowId !== activeWorkflowId) {
    throw new Error("the fixture serializes one cooperative workflow at a time");
  }

  let safePointState = terminal ?? "running";
  if (pendingPause) {
    controlEpoch = pendingPause.controlEpoch;
    pendingPause = null;
    mode = "paused";
    if (!terminal) safePointState = "paused";
  } else if (mode === "initialPause" || mode === "step") {
    mode = "paused";
    if (!terminal) safePointState = "paused";
  }

  sequence += 1;
  emit({
    nonce,
    debugSessionId,
    workflowId,
    sequence,
    controlEpoch,
    stepId,
    ...(parentStepId ? { parentStepId } : {}),
    kind,
    flowStage,
    source,
    safePointState,
    ...(operation ? { operation } : {}),
    ...(resource ? { resource } : {}),
    ...(branchOutcome ? { branchOutcome } : {}),
    ...(dataPreview ? { dataPreview } : {}),
  });
  lastStepId = stepId;
  workflowTerminalEmitted = Boolean(terminal);

  if (safePointState === "paused") {
    await new Promise((resolve) => {
      pausedResolver = resolve;
    });
  } else {
    // Yield so stdin controls remain responsive during dense instrumentation.
    await new Promise((resolve) => setImmediate(resolve));
  }
}

function emitAdapterTerminal(workflowId, terminal) {
  return debugSafePoint({
    workflowId,
    stepId: terminal === "workflowFailed" ? "adapter-failed" : "adapter-completed",
    parentStepId: lastStepId,
    kind: "response",
    flowStage: "response",
    resource: "instrumented workflow",
    source: { relativePath: "src/aone-debug.js", line: 123 },
    terminal,
  });
}

function receiveControl(line) {
  if (!line.startsWith(CONTROL_PREFIX)) return;
  let control;
  try {
    control = JSON.parse(line.slice(CONTROL_PREFIX.length));
  } catch {
    return;
  }
  if (
    control.nonce !== nonce ||
    control.debugSessionId !== debugSessionId ||
    !Number.isSafeInteger(control.controlEpoch) ||
    control.controlEpoch !== controlEpoch + 1 ||
    !Number.isSafeInteger(control.expectedSequence) ||
    control.expectedSequence > sequence
  ) {
    return;
  }

  if (control.action === "pause" && mode === "running") {
    pendingPause = control;
    return;
  }
  if (control.action === "stop") {
    if (process.env.AONE_DEBUG_IGNORE_STOP !== "1") process.exit(0);
    return;
  }
  if (
    mode !== "paused" ||
    control.expectedSequence !== sequence ||
    !["resume", "stepInto", "stepOver"].includes(control.action)
  ) {
    return;
  }

  controlEpoch = control.controlEpoch;
  mode = control.action === "resume" ? "running" : "step";
  const release = pausedResolver;
  pausedResolver = null;
  release?.();
}

function emit(envelope) {
  process.stdout.write(`${EVENT_PREFIX}${JSON.stringify(envelope)}\n`);
}
