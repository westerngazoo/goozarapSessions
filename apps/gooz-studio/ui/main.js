// goozarapSessions — studio shell v0 (frontend).
// Uses the Tauri backend when present; otherwise falls back to a baked real
// pipeline fixture (window.__GOOZ_FIXTURE__) so the UI previews in a browser.

const invoke =
  window.__TAURI__?.core?.invoke ?? window.__TAURI__?.invoke ?? null;

let current = null;
let audioCtx = null;
let node = null;
let playing = false;
let busy = false;

// What the recording becomes: "hum" runs the hum→riff pipeline (R-0008),
// "instrument" plays the take back across the ratio grid (R-0040), "style" puts
// the take over a drum track in a chosen style (R-0042).
let mode = "hum";

// The chosen style (a preset id), and the styled drums of the current result.
// While `track` is set it *is* the beat: playing, saving and exporting use it,
// and nothing fetches a generic Easy Mode beat over it.
let style = null;
let track = null;

// A capture in progress: tap to start, tap again to stop (R-0042). The shell
// caps a take at 30 s; the UI stops itself at the same point.
const MAX_RECORD_MS = 30_000;
let recording = null;

// The preset ids when there is no shell to ask (browser preview).
const PREVIEW_STYLES = ["corrido", "trap", "metal", "free"];
const STYLE_LABELS = { free: "libre" };

const MODES = {
  hum: {
    prompt: "hum something",
    hint: "tap &amp; hum a melody, tap again to stop — no theory, just a sound.",
    label: "hum",
    heard: "esto escuché",
    stop: "record_stop_analyze",
    // Hum mode keeps R-0013's graceful fallback to the demo riff.
    demo: true,
  },
  instrument: {
    prompt: "make any sound",
    hint: "tap &amp; hit the table, click, knock, tap again to stop — it becomes your instrument.",
    label: "sound",
    heard: "tu instrumento",
    stop: "record_stop_instrument",
    // No demo here: the hum demo is a guitar built from a synthetic hum, and
    // showing it under "tu instrumento" would be showing someone else's sound.
    demo: false,
  },
  style: {
    prompt: "sing something",
    hint: "pick a style, tap &amp; sing — tap again to stop. the drums follow you.",
    label: "sing",
    heard: "tú sobre el estilo",
    stop: "record_stop_accompany",
    demo: false,
    chips: true,
  },
};

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

function tenseValue() {
  return Number(document.getElementById("tenseRng").value);
}

function onDemo(e) {
  e.preventDefault();
  if (busy) return;
  busy = true;
  setModesEnabled(false);
  demo()
    .then((data) => present(data, MODES.hum))
    .finally(() => {
      busy = false;
      setModesEnabled(true);
    });
}

// The mode can only change while nothing is in flight: a result must be labelled
// by the mode that produced it, not by whichever pill was tapped since.
function setModesEnabled(on) {
  for (const btn of document.querySelectorAll(".mode, .style-chip")) btn.disabled = !on;
}

// ---- style chips: one per preset, asked from the engine ----
// One request for the style list, shared by every mode switch that asks —
// toggling twice before it answers must not add the chips twice — and
// forgotten if it fails, so the next switch tries again.
let stylesRequest = null;
function loadStyles() {
  if (stylesRequest) return stylesRequest;
  stylesRequest = (invoke ? invoke("styles") : Promise.resolve(PREVIEW_STYLES))
    .then(renderStyles)
    .catch((err) => {
      stylesRequest = null;
      showIntroMessage(`couldn't load the styles: ${err}`);
    });
  return stylesRequest;
}

function renderStyles(names) {
  const row = document.getElementById("styles");
  row.replaceChildren();
  for (const name of names) {
    const chip = document.createElement("button");
    chip.className = "style-chip";
    chip.setAttribute("role", "radio");
    chip.setAttribute("aria-checked", "false");
    chip.dataset.style = name;
    chip.textContent = STYLE_LABELS[name] || name;
    chip.addEventListener("click", () => pickStyle(name));
    row.appendChild(chip);
  }
}

function pickStyle(name) {
  if (busy || recording) return;
  style = name;
  for (const chip of document.querySelectorAll(".style-chip")) {
    const on = chip.dataset.style === name;
    chip.classList.toggle("is-on", on);
    chip.setAttribute("aria-checked", String(on));
  }
  showIntroMessage("");
}

// Something went wrong with a take: say so, on the screen the user is looking at.
function showIntroMessage(text) {
  const el = document.getElementById("introMsg");
  el.textContent = text;
  el.classList.toggle("hidden", !text);
}

// ---- mode toggle ----
// The modes stay on screen over a result too, so picking one is also the way
// back from it: to the mic, in the mode picked.
function setMode(next) {
  if (busy || recording || !MODES[next]) return;
  reset();
  mode = next;
  const copy = MODES[mode];
  document.getElementById("prompt").innerHTML =
    `${copy.prompt}<span class="cursor">_</span>`;
  document.getElementById("hint").innerHTML = copy.demo
    ? `${copy.hint} <a href="#" id="demoLink">or hear a demo ▶</a>`
    : copy.hint;
  if (copy.demo) document.getElementById("demoLink").addEventListener("click", onDemo);
  document.getElementById("recBtn").querySelector(".label").textContent = copy.label;
  document.getElementById("styles").classList.toggle("hidden", !copy.chips);
  if (copy.chips) loadStyles();
  showIntroMessage("");
  for (const btn of document.querySelectorAll(".mode")) {
    const on = btn.dataset.mode === mode;
    btn.classList.toggle("is-on", on);
    btn.setAttribute("aria-checked", String(on));
  }
}

async function demo() {
  if (invoke) return invoke("demo_riff");
  await wait(380); // pretend to think
  return window.__GOOZ_FIXTURE__;
}

// ---- record / demo ----
async function onRecord() {
  if (recording) return finishRecording();
  if (busy) return;
  const copy = MODES[mode];
  if (copy.chips && !style) return showIntroMessage("pick a style first — then sing");
  showIntroMessage("");
  if (!invoke) {
    // Browser preview: there is no microphone here.
    if (!copy.demo) return showIntroMessage("recording needs the desktop app — there is no microphone here");
    busy = true;
    setModesEnabled(false);
    try {
      await wait(1500);
      present(await demo(), MODES.hum);
    } finally {
      busy = false;
      setModesEnabled(true);
    }
    return;
  }
  busy = true;
  setModesEnabled(false);
  try {
    await invoke("record_start");
  } catch (err) {
    showIntroMessage(`couldn't start recording: ${err}`);
    busy = false;
    setModesEnabled(true);
    return;
  }
  document.body.classList.add("listening");
  document.getElementById("recBtn").querySelector(".label").textContent = "tap to stop";
  // Captured now, so the result is labelled by the mode that recorded it.
  recording = { copy, timer: setTimeout(finishRecording, MAX_RECORD_MS) };
}

async function finishRecording() {
  if (!recording) return;
  const { copy, timer } = recording;
  recording = null;
  clearTimeout(timer);
  const rec = document.getElementById("recBtn");
  rec.querySelector(".label").textContent = "working…";
  try {
    // Each stop command gets exactly its own arguments.
    const args = copy.chips ? { tense: tenseValue(), style } : { tense: tenseValue() };
    present(await invoke(copy.stop, args), copy);
  } catch (err) {
    if (copy.demo) {
      present(await demo(), MODES.hum); // R-0013's graceful fallback
    } else {
      // A typed error from the take — silence, a clipped mic, a corrupt sample.
      // The user can fix any of those by recording again, if they are told.
      showIntroMessage(`that take didn't work: ${err}`);
    }
  } finally {
    document.body.classList.remove("listening");
    rec.querySelector(".label").textContent = MODES[mode].label;
    busy = false;
    setModesEnabled(true);
  }
}

// A result from any mode. An accompaniment is two stems: the voice is shown and
// played as the riff, and its styled drums become *the* beat.
function present(result, copy) {
  if (copy.chips) {
    stopBeat();
    track = result.track;
    lastBeat = track;
    showResult(result.voice, copy.heard);
  } else {
    track = null;
    showResult(result, copy.heard);
  }
  setBeatControlsEnabled(!track);
}

// With a styled track, voice and drums are one loop started from one play
// button at one scheduled instant. The busy slider and the beat button would
// restart the drums alone "now", while the voice kept its place — out of step
// on every drag. So while there is a track, they are off.
function setBeatControlsEnabled(on) {
  document.getElementById("busyRng").disabled = !on;
  document.getElementById("beatBtn").disabled = !on;
}

// ---- render ----
function consonanceColor(num, den) {
  const t = Math.min(1, Math.log2(num * den) / Math.log2(48));
  return `hsl(${Math.round(186 + t * 140)} 88% 62%)`;
}
function noteCard(nt) {
  const el = document.createElement("div");
  el.className = "card";
  el.style.setProperty("--c", consonanceColor(nt.num, nt.den));
  const oct = nt.octave ? ` · 8ve ${nt.octave > 0 ? "+" : ""}${nt.octave}` : "";
  // A sampled card has no pitch: the sound was moved by a ratio, and nothing
  // measured what pitch it started at. Show only what is known.
  const hz = nt.hz == null ? "" : `<div class="hz">${Math.round(nt.hz)} Hz</div>`;
  const cents =
    nt.cents == null ? "" : `${nt.cents >= 0 ? "+" : ""}${Math.round(nt.cents)}¢`;
  el.innerHTML =
    `<div class="ratio">${nt.num}<span>:</span>${nt.den}</div>` +
    hz +
    `<div class="cents">${cents}${oct}</div>`;
  return el;
}
function drawWave(data) {
  const cv = document.getElementById("wave");
  const ctx = cv.getContext("2d");
  const W = cv.width, H = cv.height, mid = H / 2;
  ctx.clearRect(0, 0, W, H);
  const bars = data.bars || 1;
  ctx.strokeStyle = "rgba(148,163,184,.16)";
  ctx.lineWidth = 1;
  for (let b = 1; b < bars; b++) {
    const x = (W * b) / bars;
    ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, H); ctx.stroke();
  }
  const wave = data.wave || [];
  const n = wave.length;
  if (!n) return;
  const g = ctx.createLinearGradient(0, 0, W, 0);
  g.addColorStop(0, "#22d3ee");
  g.addColorStop(0.5, "#a855f7");
  g.addColorStop(1, "#ec4899");
  ctx.fillStyle = g;
  ctx.shadowColor = "#a855f7";
  ctx.shadowBlur = 16;
  ctx.beginPath();
  ctx.moveTo(0, mid);
  for (let i = 0; i < n; i++) ctx.lineTo((W * i) / (n - 1), mid - wave[i] * mid * 0.92);
  for (let i = n - 1; i >= 0; i--) ctx.lineTo((W * i) / (n - 1), mid + wave[i] * mid * 0.92);
  ctx.closePath();
  ctx.globalAlpha = 0.92;
  ctx.fill();
  ctx.globalAlpha = 1;
  ctx.shadowBlur = 0;
}
function showResult(data, heading) {
  current = data;
  document.getElementById("heard").textContent = heading;
  const nn = document.getElementById("notes");
  nn.innerHTML = "";
  data.notes.forEach((nt, i) => {
    const c = noteCard(nt);
    c.style.animationDelay = `${i * 70}ms`;
    nn.appendChild(c);
  });
  drawWave(data);
  const bl = document.getElementById("barlabels");
  bl.innerHTML = "";
  for (let b = 1; b <= (data.bars || 1); b++) {
    const s = document.createElement("span");
    s.textContent = `bar ${b}`;
    bl.appendChild(s);
  }
  // The tempo the riff was actually laid out at, and whose it was.
  const bpm = Math.round(data.bpm ?? 92);
  const whose =
    data.followedBpm != null ? " · tu tempo" : data.part === "voice" ? " · tempo del estilo" : "";
  document.getElementById("meta").textContent =
    `${data.bars} bars · ${(data.seconds || 0).toFixed(1)}s · ${bpm} bpm${whose}`;
  refreshBeat();
  document.getElementById("intro").classList.add("hidden");
  document.getElementById("result").classList.remove("hidden");
}
// Back to the mic, in the current mode.
function reset() {
  stopAudio();
  if (track) {
    stopBeat();
    track = null;
    setBeatControlsEnabled(true);
  }
  document.getElementById("result").classList.add("hidden");
  document.getElementById("intro").classList.remove("hidden");
}

// ---- playback (Web Audio) ----
function stopAudio() {
  if (node) { try { node.stop(); } catch (_) {} node = null; }
  playing = false;
  const b = document.getElementById("playBtn");
  if (b) b.textContent = "▶ play loop";
}
function synthBuffer(ctx, data) {
  // Preview-only: approximate the riff from the note pitches when the backend
  // did not return raw samples (browser/dev mock). Real audio comes from Tauri.
  const sr = data.sampleRate || 48000;
  const len = Math.max(1, Math.floor((data.seconds || 2) * sr));
  const buf = ctx.createBuffer(1, len, sr);
  const ch = buf.getChannelData(0);
  const notes = data.notes || [];
  const step = len / Math.max(1, notes.length);
  notes.forEach((nt, i) => {
    const start = Math.floor(i * step), dur = Math.floor(step * 0.9);
    for (let k = 0; k < dur && start + k < len; k++) {
      const t = k / sr;
      ch[start + k] += 0.28 * Math.exp(-4 * t) * Math.sin(2 * Math.PI * nt.hz * t);
    }
  });
  return buf;
}
async function togglePlay() {
  const btn = document.getElementById("playBtn");
  if (playing) {
    if (track) stopBeat();
    return stopAudio();
  }
  audioCtx = audioCtx || new (window.AudioContext || window.webkitAudioContext)();
  await audioCtx.resume();
  let buf;
  if (current.samples && current.samples.length) {
    buf = audioCtx.createBuffer(1, current.samples.length, current.sampleRate);
    buf.copyToChannel(Float32Array.from(current.samples), 0);
  } else {
    buf = synthBuffer(audioCtx, current);
  }
  node = audioCtx.createBufferSource();
  node.buffer = buf;
  node.loop = true;
  node.connect(audioCtx.destination);
  // With a styled track, both loops start at one scheduled instant. They are
  // the same length by construction (R-0042), so they stay locked together.
  const at = audioCtx.currentTime + 0.05;
  if (track) startBeatNode(track, at);
  node.start(at);
  playing = true;
  btn.textContent = "◼ stop";
}

// ---- beat builder (sparse↔busy) ----
let beatNode = null;
let beatPlaying = false;
let lastBeat = null; // the most recent BeatView, for save/export

function busyValue() {
  return Number(document.getElementById("busyRng").value);
}

// E(k, n) via Bjorklund — mirrors gooz-ratio, for the browser preview fallback.
function euclid(k, n) {
  if (n <= 0) return [];
  if (k <= 0) return Array(n).fill(false);
  if (k >= n) return Array(n).fill(true);
  let filled = Array.from({ length: k }, () => [true]);
  let rest = Array.from({ length: n - k }, () => [false]);
  while (rest.length > 1) {
    const pairs = Math.min(filled.length, rest.length);
    const next = [];
    for (let i = 0; i < pairs; i++) next.push(filled[i].concat(rest[i]));
    const left = filled.length > pairs ? filled.slice(pairs) : rest.slice(pairs);
    filled = next; rest = left;
  }
  return filled.concat(rest).flat();
}
function rotate(steps, by) {
  const n = steps.length; if (!n) return steps;
  const s = ((by % n) + n) % n;
  return steps.slice(n - s).concat(steps.slice(0, n - s));
}
function scale(min, max, b) { return Math.round(min + (max - min) * (b / 100)); }

// Backend beat when Tauri is present; otherwise a client-side synth so the
// button still works in a plain browser preview.
async function fetchBeat(busy) {
  // An accompaniment's drums are the style's; the busy slider does not swap a
  // generic Easy Mode beat in under the voice.
  if (track) return track;
  // The beat plays under the riff, so it follows whatever the riff
  // followed — otherwise a take heard at 126 BPM gets a 92 BPM loop.
  if (invoke) return invoke("beat", { busy, bpm: current?.followedBpm ?? null });
  await wait(120);
  return synthBeat(busy);
}
function synthBeat(busy) {
  const sr = 48000, bpm = 92, beatsPerBar = 4, bars = 2, steps = 16;
  const barSamples = Math.round((60 / bpm) * beatsPerBar * sr);
  const total = barSamples * bars;
  const out = new Float32Array(total);
  const lanes = [
    { name: "kick", k: scale(2, 8, busy), rot: 0, lvl: 1.0 },
    { name: "snare", k: scale(2, 4, busy), rot: 4, lvl: 0.9 },
    { name: "hat", k: scale(4, 16, busy), rot: 0, lvl: 0.7 },
  ];
  const hit = (buf, at, name, lvl) => {
    const dur = name === "hat" ? 0.05 : name === "snare" ? 0.15 : 0.2;
    const len = Math.floor(dur * sr);
    for (let i = 0; i < len && at + i < buf.length; i++) {
      const t = i / sr;
      let s;
      if (name === "kick") s = Math.sin(2 * Math.PI * (55 + 110 * Math.exp(-t * 12)) * t) * Math.exp(-t * 10);
      else if (name === "snare") s = (Math.random() * 2 - 1) * 0.7 * Math.exp(-t * 18);
      else s = (Math.random() * 2 - 1) * Math.exp(-t * 40);
      buf[at + i] += s * lvl;
    }
  };
  for (let b = 0; b < bars; b++) {
    for (const ln of lanes) {
      const pat = rotate(euclid(ln.k, steps), ln.rot);
      for (let s = 0; s < steps; s++) {
        if (!pat[s]) continue;
        hit(out, b * barSamples + Math.round((s / steps) * barSamples), ln.name, ln.lvl);
      }
    }
  }
  let peak = 0; for (const x of out) peak = Math.max(peak, Math.abs(x));
  if (peak > 0) for (let i = 0; i < out.length; i++) out[i] /= peak;
  return {
    sampleRate: sr, bars, seconds: total / sr,
    voices: lanes.map((l) => ({ name: l.name, onsets: l.k, steps })),
    samples: Array.from(out),
  };
}

function showLanes(voices) {
  document.getElementById("beatLanes").textContent =
    (voices || []).map((v) => `${v.name} ${v.onsets}/${v.steps}`).join("  ·  ");
}
function stopBeat() {
  if (beatNode) { try { beatNode.stop(); } catch (_) {} beatNode = null; }
  beatPlaying = false;
  document.getElementById("beatBtn").textContent = "▶ beat";
}
async function playBeat() {
  const data = await fetchBeat(busyValue());
  lastBeat = data;
  audioCtx = audioCtx || new (window.AudioContext || window.webkitAudioContext)();
  await audioCtx.resume();
  startBeatNode(data, audioCtx.currentTime);
}

function startBeatNode(data, at) {
  showLanes(data.voices);
  const buf = audioCtx.createBuffer(1, data.samples.length, data.sampleRate);
  buf.copyToChannel(Float32Array.from(data.samples), 0);
  if (beatNode) { try { beatNode.stop(); } catch (_) {} }
  beatNode = audioCtx.createBufferSource();
  beatNode.buffer = buf;
  beatNode.loop = true;
  beatNode.connect(audioCtx.destination);
  beatNode.start(at);
  beatPlaying = true;
  document.getElementById("beatBtn").textContent = "◼ beat";
}
// A new riff can be at a new tempo. A beat fetched for the previous one would
// play against it — and be what save/export write next to it, under settings
// that claim the new tempo. So the beat is fetched again for the new riff:
// restarted if it was playing, replaced if it was only held for save/export.
async function refreshBeat() {
  if (track) return; // the styled track is the beat; nothing to re-fetch
  if (beatPlaying) return playBeat();
  if (lastBeat) lastBeat = await fetchBeat(busyValue());
}

async function toggleBeat() {
  if (track) return; // the style's drums play with the voice, from play
  if (beatPlaying) return stopBeat();
  await playBeat();
}

// ---- save / export (gooz-session) ----
function toast(msg, ok = true) {
  const t = document.getElementById("toast");
  t.textContent = msg;
  t.classList.remove("hidden");
  t.classList.toggle("err", !ok);
}
function sessionName() {
  return "session " + new Date().toISOString().slice(0, 19).replace("T", " ");
}
async function saveOrExport(cmd, label) {
  if (!current && !lastBeat) return toast("nothing to save yet — hum or build a beat", false);
  if (!invoke) return toast("open the desktop app to " + label, false);
  const args = {
    name: sessionName(),
    tense: tenseValue(),
    busy: busyValue(),
    riff: current || null,
    beat: lastBeat || null,
  };
  try {
    const path = await invoke(cmd, args);
    toast(label + " → " + path);
  } catch (e) {
    toast(label + " failed: " + e, false);
  }
}

// ---- wire ----
document.getElementById("recBtn").addEventListener("click", onRecord);
document.getElementById("demoLink").addEventListener("click", onDemo);
for (const btn of document.querySelectorAll(".mode")) {
  btn.addEventListener("click", () => setMode(btn.dataset.mode));
}
document.getElementById("playBtn").addEventListener("click", togglePlay);
document.getElementById("redoBtn").addEventListener("click", reset);
document.getElementById("beatBtn").addEventListener("click", toggleBeat);
document.getElementById("busyRng").addEventListener("input", () => { if (beatPlaying && !track) playBeat(); });
document.getElementById("saveBtn").addEventListener("click", () => saveOrExport("save_session", "saved session"));
document.getElementById("exportBtn").addEventListener("click", () => saveOrExport("export_master", "exported wav"));
