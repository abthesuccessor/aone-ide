const manifestPath = "./downloads/manifest.json";

function humanBytes(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "Unavailable";
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}

function setText(selector, value) {
  for (const element of document.querySelectorAll(selector)) element.textContent = value;
}

function disableDownloads(message) {
  for (const link of document.querySelectorAll("[data-download-link]")) {
    link.setAttribute("aria-disabled", "true");
    link.setAttribute("href", "#download");
    link.textContent = "Build unavailable";
  }
  setText("[data-build-state]", message);
  setText("[data-download-note]", message);
}

function resolveArtifactUrl(rawUrl, publicReady) {
  if (typeof rawUrl !== "string" || rawUrl.length === 0) return null;
  const resolved = new URL(rawUrl, window.location.href);
  const secureProtocol = resolved.protocol === "https:";
  const localTestProtocol = !publicReady && resolved.protocol === "http:";
  if ((!secureProtocol && !localTestProtocol) || resolved.origin !== window.location.origin) {
    throw new Error("Artifact URL must stay on the current trusted site origin");
  }
  return resolved.href;
}

function hasAppleReleaseEvidence(manifest, artifact) {
  const verification = manifest.verification;
  return manifest.channel === "public"
    && artifact.publicReady === true
    && artifact.signed === true
    && artifact.notarized === true
    && verification?.provider === "Apple"
    && verification.bundleIdentifier === "com.aone.ide"
    && verification.version === manifest.version
    && verification.build === manifest.build
    && verification.target === artifact.target
    && verification.architecture === artifact.architecture
    && verification.minimumMacos === artifact.minimumMacos
    && /^[A-Z0-9]{10}$/.test(verification.teamId ?? "")
    && typeof verification.signingIdentity === "string"
    && verification.signingIdentity.endsWith(` (${verification.teamId})`);
}

function applyManifest(manifest) {
  const artifact = manifest.artifact;
  if (!artifact || typeof artifact !== "object") throw new Error("Artifact metadata is missing");

  setText("[data-build-size]", humanBytes(artifact.bytes));
  setText("[data-build-architecture]", artifact.architecture || "Unavailable");
  setText("[data-build-macos]", artifact.minimumMacos ? `${artifact.minimumMacos}+` : "Unavailable");
  setText("[data-build-version]", manifest.version || "Unavailable");
  setText("[data-build-signing]", artifact.signed ? "Developer ID" : "Ad-hoc only");
  setText("[data-build-notarization]", artifact.notarized ? "Apple notarized" : "Not notarized");
  setText("[data-build-published]", manifest.publishedAt ? new Date(manifest.publishedAt).toLocaleDateString() : "Not published");
  setText("[data-checksum]", artifact.sha256 || "Checksum unavailable");

  // These facts are publisher metadata, not a browser-side substitute for
  // macOS Developer ID and Gatekeeper verification of the downloaded app.
  const publicReady = hasAppleReleaseEvidence(manifest, artifact);
  const artifactUrl = resolveArtifactUrl(artifact.url, publicReady);
  const state = document.querySelector("[data-build-state]");
  state?.classList.toggle("is-public", publicReady);
  state?.classList.toggle("is-local", !publicReady);
  setText("[data-build-state]", publicReady
    ? `Public release ${manifest.version}. macOS verifies its Developer ID signature and notarization when opened.`
    : `Local test build ${manifest.version}. Gatekeeper approval may be required.`);
  setText("[data-download-note]", publicReady
    ? `Publisher metadata reports Developer ID signing and notarization for ${artifact.architecture} Macs on macOS ${artifact.minimumMacos} or later. Keep Gatekeeper enabled.`
    : `This artifact is for local testing. Public distribution still requires Developer ID signing and Apple notarization.`);

  if (!artifactUrl) {
    disableDownloads("No downloadable artifact has been staged for this site build.");
    return;
  }

  for (const link of document.querySelectorAll("[data-download-link]")) {
    link.removeAttribute("aria-disabled");
    link.setAttribute("href", artifactUrl);
    link.setAttribute("download", artifact.filename || "");
    link.textContent = publicReady ? "Download for Mac" : "Download local test";
  }

  const copyButton = document.querySelector("[data-copy-checksum]");
  if (copyButton instanceof HTMLButtonElement && artifact.sha256) {
    copyButton.disabled = false;
    copyButton.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(artifact.sha256);
        copyButton.textContent = "Checksum copied";
      } catch {
        copyButton.textContent = "Copy unavailable";
      }
    });
  }
}

fetch(manifestPath, { cache: "no-store" })
  .then((response) => {
    if (!response.ok) throw new Error(`Manifest request failed with ${response.status}`);
    return response.json();
  })
  .then(applyManifest)
  .catch(() => disableDownloads("Release metadata is unavailable or inconsistent. No download is offered."));
