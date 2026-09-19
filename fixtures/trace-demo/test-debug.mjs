import assert from "node:assert/strict";
import readline from "node:readline";
import { spawn } from "node:child_process";

const EVENT_PREFIX = "AONE_DEBUG_V1 ";
const CONTROL_PREFIX = "AONE_DEBUG_CONTROL_V1 ";
const nonce = "fixture-nonce";
const debugSessionId = "fixture-session";
const child = spawn(process.execPath, ["src/server.js"], {
  cwd: new URL(".", import.meta.url),
  detached: process.platform !== "win32",
  stdio: ["pipe", "pipe", "pipe"],
  env: {
    ...process.env,
    PORT: "0",
    AONE_DEBUG_PROTOCOL: "AONE_DEBUG_V1",
    AONE_DEBUG_NONCE: nonce,
    AONE_DEBUG_SESSION_ID: debugSessionId,
    AONE_DEBUG_IGNORE_STOP: "1",
  },
});

const events = [];
const waiters = new Set();
let port;
let stderr = "";
readline.createInterface({ input: child.stdout }).on("line", (line) => {
  if (line.startsWith(EVENT_PREFIX)) {
    const event = JSON.parse(line.slice(EVENT_PREFIX.length));
    events.push(event);
    for (const wake of waiters) wake();
    return;
  }
  const match = line.match(/127\.0\.0\.1:(\d+)$/);
  if (match) {
    port = Number(match[1]);
    for (const wake of waiters) wake();
  }
});
child.stderr.on("data", (chunk) => {
  stderr += chunk.toString();
});

try {
  await waitUntil(() => port, "server listening");
  const baseUrl = `http://127.0.0.1:${port}`;

  let firstSettled = false;
  const firstRequest = fetch(`${baseUrl}/api/shipments/SHP-1047`).finally(() => {
    firstSettled = true;
  });
  const firstPause = await waitEvent((event) => event.sequence === 1);
  assert.equal(firstPause.safePointState, "paused");
  assert.equal(firstPause.stepId, "route-entry");
  await delay(100);
  assert.equal(events.length, 1, "acknowledged pause must not make progress");
  assert.equal(firstSettled, false, "HTTP response must remain blocked at the safe point");

  sendControl("stepOver", 1, 1);
  const stepped = await waitEvent((event) => event.sequence === 2);
  assert.equal(stepped.safePointState, "paused");
  await delay(100);
  assert.equal(events.length, 2, "step must release exactly one safe point");

  sendControl("resume", 2, 2);
  const firstResponse = await firstRequest;
  assert.equal(firstResponse.status, 200);
  const firstTerminal = await waitEvent(
    (event) => event.workflowId === firstPause.workflowId && event.kind === "response",
  );
  assert.equal(firstTerminal.safePointState, "workflowCompleted");
  assert.equal(JSON.stringify(events).includes("SHP-1047"), false, "payload values must not emit");

  const secondRequest = fetch(`${baseUrl}/api/shipments/SHP-1047`);
  const secondPause = await waitEvent(
    (event) => event.workflowId !== firstPause.workflowId && event.stepId === "route-entry",
  );
  assert.equal(secondPause.safePointState, "paused");
  assert.equal(secondPause.stepId, firstPause.stepId, "stable checkpoint ids may repeat by workflow");
  sendControl("resume", secondPause.sequence, 3);
  assert.equal((await secondRequest).status, 200);
  await waitEvent(
    (event) => event.workflowId === secondPause.workflowId && event.kind === "response",
  );

  const failedRequest = fetch(`${baseUrl}/api/debug/fail`);
  const failedPause = await waitEvent((event) => event.stepId === "failure-entry");
  assert.equal(failedPause.safePointState, "paused");
  sendControl("resume", failedPause.sequence, 4);
  assert.equal((await failedRequest).status, 500);
  const failedTerminal = await waitEvent(
    (event) =>
      event.workflowId === failedPause.workflowId &&
      event.safePointState === "workflowFailed",
  );
  assert.equal(failedTerminal.stepId, "adapter-failed");

  const stressRequest = fetch(`${baseUrl}/api/debug/stress`);
  const stressPause = await waitEvent((event) => event.stepId === "stress-entry");
  assert.equal(stressPause.safePointState, "paused");
  sendControl("resume", stressPause.sequence, 5);
  const runningBurst = await waitEvent((event) => event.stepId === "stress-line-10");
  sendControl("pause", runningBurst.sequence, 6);
  const burstPause = await waitEvent(
    (event) =>
      event.workflowId === stressPause.workflowId &&
      event.sequence > runningBurst.sequence &&
      event.safePointState === "paused",
  );
  const pausedCount = events.length;
  await delay(100);
  assert.equal(events.length, pausedCount, "high-rate child must still acknowledge pause");
  sendControl("resume", burstPause.sequence, 7);
  assert.equal((await stressRequest).status, 204);
  await waitEvent((event) => event.stepId === "stress-response");
  const stressEvents = events.filter((event) => event.workflowId === stressPause.workflowId);
  assert.equal(stressEvents.length, 122);

  const finalSequence = events.at(-1).sequence;
  sendControl("stop", finalSequence, 8);
  await delay(100);
  assert.equal(child.exitCode, null, "fixture intentionally ignores cooperative stop");
  if (process.platform === "win32") child.kill("SIGTERM");
  else process.kill(-child.pid, "SIGTERM");
  await waitForExit();
  assert.notEqual(child.exitCode, null, "authoritative OS signal must end the child");

  console.log(`debug fixture passed: ${events.length} safe points, serialized 4 workflows`);
} catch (error) {
  if (child.exitCode === null) {
    if (process.platform === "win32") child.kill("SIGKILL");
    else process.kill(-child.pid, "SIGKILL");
  }
  throw new Error(`${error.message}\nchild stderr:\n${stderr}`, { cause: error });
}

function sendControl(action, expectedSequence, controlEpoch) {
  child.stdin.write(
    `${CONTROL_PREFIX}${JSON.stringify({
      nonce,
      debugSessionId,
      action,
      controlEpoch,
      expectedSequence,
    })}\n`,
  );
}

async function waitEvent(predicate) {
  return waitUntil(() => events.find(predicate), "debug safe point");
}

async function waitUntil(probe, label) {
  const deadline = Date.now() + 5_000;
  while (Date.now() < deadline) {
    const value = probe();
    if (value) return value;
    await new Promise((resolve) => {
      const timer = setTimeout(() => {
        waiters.delete(wake);
        resolve();
      }, 25);
      const wake = () => {
        clearTimeout(timer);
        waiters.delete(wake);
        resolve();
      };
      waiters.add(wake);
    });
  }
  throw new Error(`timed out waiting for ${label}`);
}

function waitForExit() {
  if (child.exitCode !== null) return Promise.resolve();
  return new Promise((resolve) => child.once("exit", resolve));
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
