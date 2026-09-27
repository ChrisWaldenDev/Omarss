#!/usr/bin/env node
// Driver for the omarss Tauri app, for agents.
//
// Launches a debug build on a hidden Hyprland special workspace (no window on the user's
// screen, no focus stealing) with an isolated data dir, then drives it through WebKitGTK's
// remote inspector. One command per invocation; the app keeps running between commands.
//
//   node .claude/skills/run-omarss/driver.mjs <command> [args]    (run from the repo root)
//
// Requires Node >= 22.5 (global WebSocket, node:sqlite) and a Hyprland session.

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const UNIT = path.resolve(import.meta.dirname, "../../..");
const DIR = process.env.OMARSS_DRIVER_DIR ?? "/tmp/omarss-driver";
const PORT = Number(process.env.OMARSS_INSPECTOR_PORT ?? 9227);
const WORKSPACE = "special:omarss-driver";
const XDG_DATA = path.join(DIR, "data"); // XDG_DATA_HOME given to the app
const APP_DATA = path.join(XDG_DATA, "omarss"); // what the app creates inside it
const DB = path.join(APP_DATA, "omarss.sqlite");
const SHOTS = path.join(DIR, "shots");
const LOG = path.join(DIR, "app.log");
const LAUNCH_SCRIPT = path.join(DIR, "launch.sh");
// Own cargo target dir: `tauri dev` builds target/debug/omarss as a dev-server binary (loads
// http://localhost:1420), which would overwrite a `tauri build --debug` binary and vice versa.
const TARGET_DIR = path.join(UNIT, "src-tauri/target/driver");
const BINARY = path.join(TARGET_DIR, "debug/omarss");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const ANSI_ESCAPE = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*[A-Za-z]`, "g");
const shq = (s) => `'${String(s).replace(/'/g, `'\\''`)}'`;

function fail(message) {
  console.error(`ERROR: ${message}`);
  process.exit(1);
}

// ---------------------------------------------------------------- Hyprland / process helpers

function hyprctl(...args) {
  const r = spawnSync("hyprctl", args, { encoding: "utf8" });
  if (r.error) fail(`hyprctl not available (${r.error.message}). This driver needs Hyprland.`);
  return r.stdout.trim();
}

/** Windows the driver launched (they live on its special workspace). */
function driverWindows() {
  const clients = JSON.parse(hyprctl("clients", "-j") || "[]");
  return clients.filter((c) => c.workspace?.name === WORKSPACE);
}

async function inspectorUp() {
  try {
    const res = await fetch(`http://127.0.0.1:${PORT}/`, { signal: AbortSignal.timeout(1000) });
    return res.ok;
  } catch {
    return false;
  }
}

function tailLog(lines = 30) {
  if (!fs.existsSync(LOG)) return "(no log yet)";
  // Strip ANSI colours from tracing/cargo output.
  const text = fs.readFileSync(LOG, "utf8").replace(ANSI_ESCAPE, "");
  return text
    .split(/\r?\n|\r/)
    .filter(Boolean)
    .slice(-lines)
    .join("\n");
}

// ---------------------------------------------------------------- WebKit inspector session

/**
 * Opens an inspector connection to the app's page and runs `fn` with `send(method, params)`.
 * WebKit multiplexes targets: every command is wrapped in Target.sendMessageToTarget and
 * replies arrive as Target.dispatchMessageFromTarget.
 */
async function session(fn) {
  let html;
  try {
    html = await (await fetch(`http://127.0.0.1:${PORT}/`)).text();
  } catch {
    fail("app is not running (no inspector on port " + PORT + "). Run `launch` first.");
  }
  const socketPath = html.match(/\/socket\/\d+\/\d+\/WebPage/)?.[0];
  if (!socketPath) fail("inspector has no page yet; the app may still be starting.");

  const ws = new WebSocket(`ws://127.0.0.1:${PORT}${socketPath}`);
  const pending = new Map();
  const events = [];
  let targetId = null;
  let nextId = 1;

  const ready = new Promise((resolve, reject) => {
    ws.onerror = () => reject(new Error("inspector websocket error"));
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.method === "Target.targetCreated" && !targetId) {
        targetId = msg.params.targetInfo.targetId;
        resolve();
      } else if (msg.method === "Target.dispatchMessageFromTarget") {
        const inner = JSON.parse(msg.params.message);
        const waiter = inner.id !== undefined && pending.get(inner.id);
        if (waiter) {
          pending.delete(inner.id);
          if (inner.error)
            waiter.reject(new Error(inner.error.message ?? JSON.stringify(inner.error)));
          else waiter.resolve(inner.result);
        } else if (inner.method) {
          events.push(inner);
        }
      }
    };
  });

  const timeout = (ms, what) =>
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error(`timed out: ${what}`)), ms).unref(),
    );
  await Promise.race([ready, timeout(5000, "inspector page target")]);

  const send = (method, params = {}) => {
    const id = nextId++;
    const message = JSON.stringify({ id, method, params });
    ws.send(
      JSON.stringify({
        id: 1_000_000 + id,
        method: "Target.sendMessageToTarget",
        params: { targetId, message },
      }),
    );
    return Promise.race([
      new Promise((resolve, reject) => pending.set(id, { resolve, reject })),
      timeout(15000, method),
    ]);
  };

  try {
    return await fn({ send, events });
  } finally {
    ws.close();
  }
}

/** Evaluates a JS expression in the page. Promises are awaited; results come back as JSON. */
async function evaluate(send, expression) {
  // WebKit's Runtime.evaluate has no `awaitPromise` flag: evaluate, then awaitPromise.
  const res = await send("Runtime.evaluate", { expression, emulateUserGesture: true });
  if (res.wasThrown) throw new Error(res.result.description ?? "evaluation threw");
  const obj = res.result;
  if (obj.className === "Promise" || obj.subtype === "promise") {
    const settled = await send("Runtime.awaitPromise", {
      promiseObjectId: obj.objectId,
      returnByValue: true,
    });
    if (settled.wasThrown) {
      throw new Error(settled.result.description ?? JSON.stringify(settled.result.value));
    }
    return settled.result.value;
  }
  if (obj.objectId) {
    const byValue = await send("Runtime.callFunctionOn", {
      objectId: obj.objectId,
      functionDeclaration: "function () { return this; }",
      returnByValue: true,
    });
    return byValue.result.value;
  }
  return obj.type === "undefined" ? undefined : obj.value;
}

const print = (value) =>
  console.log(value === undefined ? "undefined" : JSON.stringify(value, null, 2));

// ---------------------------------------------------------------- commands

const COMMANDS = {
  async launch(args) {
    const dev = args.includes("--dev");
    if (!process.env.HYPRLAND_INSTANCE_SIGNATURE) {
      fail("not in a Hyprland session; this driver only knows how to launch hidden on Hyprland.");
    }
    if (driverWindows().length > 0 || (await inspectorUp())) {
      fail("already running (or port " + PORT + " is busy). Run `quit` first.");
    }
    if (args.includes("--fresh")) fs.rmSync(XDG_DATA, { recursive: true, force: true });
    fs.mkdirSync(XDG_DATA, { recursive: true });

    if (!dev && !args.includes("--no-build")) {
      console.log("building debug app (npx tauri build --debug --no-bundle)...");
      const build = spawnSync("npx", ["tauri", "build", "--debug", "--no-bundle"], {
        cwd: UNIT,
        encoding: "utf8",
        env: { ...process.env, CARGO_TARGET_DIR: TARGET_DIR },
      });
      if (build.status !== 0) {
        const out = (build.stdout + build.stderr).split("\n").slice(-40).join("\n");
        fail(`build failed:\n${out}`);
      }
    }
    if (!dev && !fs.existsSync(BINARY)) fail(`no binary at ${BINARY}; run without --no-build`);

    // Processes started by Hyprland don't inherit this shell's environment, so bake in PATH.
    const command = dev ? "npx tauri dev" : shq(BINARY);
    fs.writeFileSync(
      LAUNCH_SCRIPT,
      [
        "#!/bin/sh",
        `cd ${shq(UNIT)} || exit 1`,
        `export PATH=${shq(process.env.PATH)}`,
        `export XDG_DATA_HOME=${shq(XDG_DATA)}`,
        `export WEBKIT_INSPECTOR_HTTP_SERVER=127.0.0.1:${PORT}`,
        `exec ${command} > ${shq(LOG)} 2>&1`,
        "",
      ].join("\n"),
      { mode: 0o755 },
    );
    fs.rmSync(LOG, { force: true });

    // Hyprland >= 0.55 takes Lua; the window rule puts the window on a hidden special
    // workspace, floating at the size from tauri.conf.json.
    const rules = `{ workspace = "${WORKSPACE} silent", float = true }`;
    const out = hyprctl("dispatch", `hl.dsp.exec_cmd(${JSON.stringify(LAUNCH_SCRIPT)}, ${rules})`);
    if (out !== "ok") fail(`hyprctl dispatch failed: ${out}`);

    const deadline = Date.now() + (dev ? 600_000 : 60_000);
    while (!(await inspectorUp())) {
      if (Date.now() > deadline) fail(`app did not start. Log tail:\n${tailLog()}`);
      await sleep(500);
    }
    // Wait until the Svelte app has mounted.
    let mounted = false;
    while (!mounted && Date.now() < deadline) {
      mounted = await session(({ send }) =>
        evaluate(send, "(document.getElementById('app')?.childElementCount ?? 0) > 0"),
      ).catch(() => false);
      if (!mounted) await sleep(300);
    }
    if (!mounted) fail(`app window opened but the UI never mounted. Log tail:\n${tailLog()}`);

    const win = driverWindows()[0];
    console.log(
      `launched${dev ? " (dev mode)" : ""}: pid ${win?.pid ?? "?"}, window ${win?.size?.join("x") ?? "?"}`,
    );
    console.log(`data dir: ${APP_DATA}\nlog: ${LOG}`);
  },

  async status() {
    const wins = driverWindows();
    console.log(
      JSON.stringify(
        {
          running: wins.length > 0,
          windows: wins.map((w) => ({ pid: w.pid, title: w.title, size: w.size })),
          inspector: (await inspectorUp()) ? `http://127.0.0.1:${PORT}/` : null,
          dataDir: APP_DATA,
          log: LOG,
          shots: SHOTS,
        },
        null,
        2,
      ),
    );
  },

  async eval(args) {
    const expression = args.join(" ");
    if (!expression) fail("usage: eval <js expression>");
    print(await session(({ send }) => evaluate(send, expression)));
  },

  async click(args) {
    const selector = args.join(" ");
    if (!selector) fail("usage: click <css selector>");
    const js = `(() => { const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return "NOT_FOUND"; el.click(); return "OK: " + el.tagName.toLowerCase(); })()`;
    const result = await session(({ send }) => evaluate(send, js));
    console.log(`click ${selector} -> ${result}`);
    if (result === "NOT_FOUND") process.exitCode = 1;
    await sleep(250); // let the IPC round-trip and re-render settle before the next command
  },

  async "click-text"(args) {
    const text = args.join(" ");
    if (!text) fail("usage: click-text <visible text>");
    const js = `(() => { const t = ${JSON.stringify(text)};
      const els = [...document.querySelectorAll('button, a, label, [role="button"]')];
      const el = els.find(e => e.innerText?.trim() === t) ?? els.find(e => e.innerText?.includes(t));
      if (!el) return "NOT_FOUND"; el.click(); return "OK: " + el.tagName.toLowerCase(); })()`;
    const result = await session(({ send }) => evaluate(send, js));
    console.log(`click-text ${JSON.stringify(text)} -> ${result}`);
    if (result === "NOT_FOUND") process.exitCode = 1;
    await sleep(250);
  },

  async text(args) {
    const selector = args.join(" ");
    const js = `(${selector ? `document.querySelector(${JSON.stringify(selector)})` : "document.body"})?.innerText ?? "(not found)"`;
    console.log(await session(({ send }) => evaluate(send, js)));
  },

  async ss(args) {
    const name = args[0] ?? `ss-${Date.now()}`;
    fs.mkdirSync(SHOTS, { recursive: true });
    const file = path.join(SHOTS, `${name}.png`);
    await session(async ({ send }) => {
      const { w, h } = await evaluate(send, "({ w: innerWidth, h: innerHeight })");
      const res = await send("Page.snapshotRect", {
        x: 0,
        y: 0,
        width: w,
        height: h,
        coordinateSystem: "Viewport",
      });
      fs.writeFileSync(file, Buffer.from(res.dataURL.split(",")[1], "base64"));
      console.log(`screenshot: ${file} (${w}x${h})`);
    });
  },

  async console() {
    await session(async ({ send, events }) => {
      await send("Console.enable"); // replays messages logged before we connected
      await sleep(500);
      const lines = events
        .filter((e) => e.method === "Console.messageAdded")
        .map(({ params: { message: m } }) => `${m.level}: ${m.text}`);
      console.log(lines.join("\n") || "(no console messages)");
      await send("Console.disable");
    });
  },

  async invoke(args) {
    const [command, json = "{}"] = [args[0], args.slice(1).join(" ") || undefined];
    if (!command) fail("usage: invoke <command_name> [json args]");
    JSON.parse(json); // validate early
    const js = `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${json})
      .then(ok => ({ ok }), error => ({ error }))`;
    print(await session(({ send }) => evaluate(send, js)));
  },

  async db(args) {
    const sql = args.join(" ");
    if (!sql) fail("usage: db <sql>");
    if (!fs.existsSync(DB)) fail(`no database at ${DB}; launch the app first`);
    const { DatabaseSync } = await import("node:sqlite");
    const db = new DatabaseSync(DB, { readOnly: true });
    try {
      print(
        db
          .prepare(sql)
          .all()
          .map((row) => ({ ...row })),
      );
    } finally {
      db.close();
    }
  },

  async log(args) {
    console.log(tailLog(Number(args[0] ?? 30)));
  },

  async quit() {
    const wins = driverWindows();
    for (const w of wins) {
      try {
        process.kill(w.pid, "SIGTERM");
      } catch {
        // already gone
      }
    }
    const deadline = Date.now() + 10_000;
    while ((driverWindows().length > 0 || (await inspectorUp())) && Date.now() < deadline) {
      await sleep(250);
    }
    const stillUp = driverWindows().length > 0 || (await inspectorUp());
    console.log(stillUp ? "WARNING: app still running" : `stopped (${wins.length} window(s))`);
  },

  help() {
    console.log(`usage: node .claude/skills/run-omarss/driver.mjs <command> [args]

  launch [--dev] [--fresh] [--no-build]   build (debug) and start hidden; --dev = npx tauri dev
  status                                  is it running? paths for data, log, screenshots
  eval <js>                               evaluate an expression in the page (promises awaited)
  click <css>                             DOM-click the first match
  click-text <text>                       DOM-click a button/link/label by visible text
  text [css]                              innerText of an element (default: body)
  ss [name]                               screenshot -> ${SHOTS}/<name>.png
  console                                 console messages logged so far
  invoke <command> [json]                 call a backend command over IPC, print {ok} or {error}
  db <sql>                                read-only query against the app's SQLite database
  log [n]                                 last n lines of the app's stdout/stderr (default 30)
  quit                                    stop the app`);
  },
};

const [command = "help", ...args] = process.argv.slice(2);
const handler = COMMANDS[command];
if (!handler) fail(`unknown command "${command}"; try: help`);
try {
  await handler(args);
} catch (error) {
  fail(error.message);
}
