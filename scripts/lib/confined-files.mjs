import { constants as fsConstants, createReadStream } from "node:fs";
import { lstat, open, realpath } from "node:fs/promises";
import { createHash } from "node:crypto";
import { basename, dirname, resolve } from "node:path";

import { validateArtifactFilename } from "./release-metadata.mjs";

export async function openRegularNoFollow(path) {
  const absolutePath = resolve(path);
  const before = await lstat(absolutePath);
  if (before.isSymbolicLink()) throw new Error(`Symbolic links are not allowed: ${absolutePath}`);
  if (!before.isFile()) throw new Error(`Expected a regular file: ${absolutePath}`);

  const handle = await open(absolutePath, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
  try {
    const after = await handle.stat();
    if (!after.isFile() || before.dev !== after.dev || before.ino !== after.ino) {
      throw new Error(`File changed while it was being opened: ${absolutePath}`);
    }
    return { absolutePath, handle, stats: after };
  } catch (error) {
    await handle.close();
    throw error;
  }
}

export async function openConfinedRegularFile(path, expectedParent, { requireDmg = false } = {}) {
  const absolutePath = resolve(path);
  const absoluteParent = resolve(expectedParent);
  if (dirname(absolutePath) !== absoluteParent) {
    throw new Error(`File must be a direct child of ${absoluteParent}`);
  }
  if (requireDmg) validateArtifactFilename(basename(absolutePath));

  const parentInfo = await lstat(absoluteParent);
  if (parentInfo.isSymbolicLink() || !parentInfo.isDirectory()) {
    throw new Error(`Expected a real directory: ${absoluteParent}`);
  }
  const [parentRealPath, opened] = await Promise.all([
    realpath(absoluteParent),
    openRegularNoFollow(absolutePath),
  ]);

  try {
    const fileRealPath = await realpath(absolutePath);
    if (dirname(fileRealPath) !== parentRealPath) {
      throw new Error(`File resolves outside ${absoluteParent}`);
    }
    return { ...opened, realPath: fileRealPath, parentRealPath };
  } catch (error) {
    await opened.handle.close();
    throw error;
  }
}

export async function inspectConfinedRegularFile(path, expectedParent, options = {}) {
  const opened = await openConfinedRegularFile(path, expectedParent, options);
  try {
    return {
      absolutePath: opened.absolutePath,
      realPath: opened.realPath,
      bytes: opened.stats.size,
      mode: opened.stats.mode,
    };
  } finally {
    await opened.handle.close();
  }
}

export async function readConfinedRegularFile(path, expectedParent, options = {}) {
  const opened = await openConfinedRegularFile(path, expectedParent, options);
  try {
    return await opened.handle.readFile();
  } finally {
    await opened.handle.close();
  }
}

export async function sha256ConfinedFile(path, expectedParent, options = {}) {
  const opened = await openConfinedRegularFile(path, expectedParent, options);
  try {
    const digest = createHash("sha256");
    for await (const chunk of createReadStream(opened.absolutePath, {
      fd: opened.handle.fd,
      autoClose: false,
      start: 0,
    })) {
      digest.update(chunk);
    }
    return {
      sha256: digest.digest("hex"),
      bytes: opened.stats.size,
      absolutePath: opened.absolutePath,
      realPath: opened.realPath,
    };
  } finally {
    await opened.handle.close();
  }
}

export async function copyConfinedFileExclusive(sourcePath, sourceParent, destinationPath) {
  const source = await openConfinedRegularFile(sourcePath, sourceParent, { requireDmg: true });
  return copyOpenedFileExclusive(source, destinationPath);
}

async function copyOpenedFileExclusive(source, destinationPath) {
  let destination;
  try {
    destination = await open(
      destinationPath,
      fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL | fsConstants.O_NOFOLLOW,
      0o644,
    );
    const buffer = Buffer.allocUnsafe(1024 * 1024);
    let position = 0;
    while (true) {
      const { bytesRead } = await source.handle.read(buffer, 0, buffer.length, position);
      if (bytesRead === 0) break;
      let written = 0;
      while (written < bytesRead) {
        const result = await destination.write(buffer, written, bytesRead - written, position + written);
        if (result.bytesWritten <= 0) throw new Error("Artifact copy made no forward progress");
        written += result.bytesWritten;
      }
      position += bytesRead;
    }
    await destination.sync();
  } finally {
    await Promise.allSettled([source.handle.close(), destination?.close()]);
  }
}

export async function copyRegularFileExclusive(sourcePath, destinationPath) {
  const source = await openRegularNoFollow(sourcePath);
  return copyOpenedFileExclusive(source, destinationPath);
}
