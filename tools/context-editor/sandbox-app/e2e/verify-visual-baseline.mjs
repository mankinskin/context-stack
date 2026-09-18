#!/usr/bin/env node
// Deterministic visual-baseline capture/verification for the context-editor
// sandbox-app. W3 foundation script — see
// transcripts/18-09-2026_context-editor-sandbox-architecture/03-build-validation-foundation.md
// and ticket d320e80d for the requirement this implements.
//
// Mechanism (explicit, reproducible):
//   1. Spawn `trunk serve` in this process for a single deterministic session
//      (uses the crate's committed Trunk.toml port; no ambient dev server is
//      assumed to already be running).
//   2. Poll the served URL until it responds, with a bounded timeout.
//   3. Connect a headless Chromium instance (Playwright) to that URL.
//   4. Wait for the sandbox `<canvas>` to appear, then wait a bounded number
//      of animation frames so the kernel's per-frame SVO paged-upload +
//      double-buffer swap (see kernel/src/svo/upload.rs) has completed at
//      least once before the capture is taken — this is the "exact
//      per-frame take_dirty_ranges sequence" determinism requirement from
//      ARTIFACTS.md §7: capturing before the first swap would show stale
//      (pre-upload) buffer state.
//   5. Screenshot the canvas and compare its exact byte hash against a
//      committed baseline; create the baseline on first run.
//
// Documented limitation: comparison is an exact SHA-256 byte hash of the PNG,
// not a perceptual/pixel-tolerance diff. Any renderer nondeterminism (GPU
// driver, anti-aliasing jitter, font hinting, OS) will register as a
// mismatch even with no functional change. This harness stands up the
// baseline and detects *any* pixel-level change; a tolerant perceptual diff
// is out of scope for this waypoint (no defect-fix or optimization work is
// in scope here).
//
// This script never silently skips: a missing `trunk` binary, a server that
// never becomes ready, or a browser/canvas that never appears is reported as
// a concrete non-zero-exit blocker, not treated as a pass.

import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SANDBOX_APP_DIR = path.resolve(__dirname, '..');
const BASELINE_DIR = path.join(__dirname, 'baselines');
const BASELINE_NAME = 'sandbox-default-scene';
const BASELINE_PNG = path.join(BASELINE_DIR, `${BASELINE_NAME}.png`);
const BASELINE_JSON = path.join(BASELINE_DIR, `${BASELINE_NAME}.json`);
const ACTUAL_PNG = path.join(BASELINE_DIR, `${BASELINE_NAME}.actual.png`);

const TRUNK_PORT = 8090; // must match sandbox-app/Trunk.toml [serve].port
const BASE_URL = `http://127.0.0.1:${TRUNK_PORT}`;
const VIEWPORT = { width: 960, height: 640 };
const SERVER_READY_TIMEOUT_MS = 60_000;
const SETTLE_FRAME_COUNT = 30; // frames to await after canvas appears

function fail(message) {
  console.error(`BLOCKER: ${message}`);
  process.exitCode = 1;
}

function waitForServer(url, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    const attempt = () => {
      const req = http.get(url, (res) => {
        res.resume();
        resolve();
      });
      req.on('error', () => {
        if (Date.now() > deadline) {
          reject(
            new Error(
              `trunk serve did not respond at ${url} within ${timeoutMs}ms`,
            ),
          );
          return;
        }
        setTimeout(attempt, 500);
      });
    };
    attempt();
  });
}

async function main() {
  mkdirSync(BASELINE_DIR, { recursive: true });

  let trunkProc;
  try {
    trunkProc = spawn('trunk', ['serve', '--port', String(TRUNK_PORT)], {
      cwd: SANDBOX_APP_DIR,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
  } catch (err) {
    fail(
      `failed to spawn "trunk serve" (is trunk installed and on PATH?): ${err.message}`,
    );
    return;
  }

  let trunkStderr = '';
  trunkProc.stderr?.on('data', (chunk) => {
    trunkStderr += chunk.toString();
  });

  const trunkExitedEarly = new Promise((resolve) => {
    trunkProc.once('exit', (code, signal) => resolve({ code, signal }));
  });

  try {
    const readyOrExit = await Promise.race([
      waitForServer(BASE_URL, SERVER_READY_TIMEOUT_MS).then(() => 'ready'),
      trunkExitedEarly.then((r) => ({ exitedEarly: r })),
    ]);

    if (readyOrExit && readyOrExit.exitedEarly) {
      const { code, signal } = readyOrExit.exitedEarly;
      fail(
        `trunk serve exited before the server became ready (code=${code}, signal=${signal}). stderr:\n${trunkStderr}`,
      );
      return;
    }

    let chromium;
    try {
      ({ chromium } = await import('@playwright/test'));
    } catch (err) {
      fail(
        `@playwright/test is not installed in ${path.join(
          __dirname,
        )} — run "npm install" in this directory first: ${err.message}`,
      );
      return;
    }

    let browser;
    try {
      browser = await chromium.launch({ headless: true });
    } catch (err) {
      fail(
        `failed to launch headless Chromium (browser binaries may be missing — run "npx playwright install chromium"): ${err.message}`,
      );
      return;
    }

    try {
      const page = await browser.newPage({ viewport: VIEWPORT });
      await page.goto(BASE_URL, { waitUntil: 'load', timeout: 30_000 });

      let canvas;
      try {
        canvas = page.locator('canvas').first();
        await canvas.waitFor({ state: 'attached', timeout: 30_000 });
      } catch (err) {
        fail(
          `sandbox-app canvas never appeared within 30s at ${BASE_URL}: ${err.message}`,
        );
        return;
      }

      // Wait for SETTLE_FRAME_COUNT animation frames so at least one full
      // svo_paged_upload_system → double_buffer_swap_system cycle has run
      // before capture (see module doc above).
      await page.evaluate((frames) => {
        return new Promise((resolve) => {
          let remaining = frames;
          function step() {
            remaining -= 1;
            if (remaining <= 0) {
              resolve();
            } else {
              requestAnimationFrame(step);
            }
          }
          requestAnimationFrame(step);
        });
      }, SETTLE_FRAME_COUNT);

      const capture = await canvas.screenshot();
      const hash = createHash('sha256').update(capture).digest('hex');

      if (!existsSync(BASELINE_PNG) || !existsSync(BASELINE_JSON)) {
        writeFileSync(BASELINE_PNG, capture);
        writeFileSync(
          BASELINE_JSON,
          JSON.stringify(
            {
              sha256: hash,
              viewport: VIEWPORT,
              settleFrames: SETTLE_FRAME_COUNT,
              capturedAt: new Date().toISOString(),
              comparisonMethod: 'exact-sha256-of-png-bytes',
            },
            null,
            2,
          ) + '\n',
        );
        console.log(
          `Baseline created: ${BASELINE_PNG} (sha256=${hash}). Re-run to verify against it.`,
        );
        return;
      }

      const baselineMeta = JSON.parse(readFileSync(BASELINE_JSON, 'utf8'));
      if (hash === baselineMeta.sha256) {
        console.log(
          `Visual baseline verified: ${BASELINE_NAME} matches (sha256=${hash}).`,
        );
      } else {
        writeFileSync(ACTUAL_PNG, capture);
        fail(
          `visual baseline mismatch for ${BASELINE_NAME}: expected sha256=${baselineMeta.sha256}, got sha256=${hash}. ` +
            `Actual capture written to ${ACTUAL_PNG} for inspection. ` +
            `Comparison is exact-byte (see module doc): a mismatch may reflect real visual change OR renderer/environment nondeterminism.`,
        );
      }
    } finally {
      await browser.close();
    }
  } finally {
    trunkProc.kill();
  }
}

main().catch((err) => {
  fail(`unexpected error: ${err.stack || err.message}`);
});
