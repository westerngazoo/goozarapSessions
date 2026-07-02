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

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

async function demo() {
  if (invoke) return invoke("demo_riff");
  await wait(380); // pretend to think
  return window.__GOOZ_FIXTURE__;
}

// ---- record / demo ----
async function onRecord() {
  if (busy) return;
  busy = true;
  const rec = document.getElementById("recBtn");
  document.body.classList.add("listening");
  rec.querySelector(".label").textContent = "listening…";
  try {
    if (invoke) {
      await invoke("record_start");
      await wait(3500); // ~3.5s to hum a melody
      showResult(await invoke("record_stop_analyze"));
    } else {
      await wait(1500);
      showResult(await demo());
    }
  } catch (_) {
    showResult(await demo()); // graceful fallback
  } finally {
    document.body.classList.remove("listening");
    rec.querySelector(".label").textContent = "hum";
    busy = false;
  }
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
  const cents = (nt.cents >= 0 ? "+" : "") + Math.round(nt.cents);
  const oct = nt.octave ? ` · 8ve ${nt.octave > 0 ? "+" : ""}${nt.octave}` : "";
  el.innerHTML =
    `<div class="ratio">${nt.num}<span>:</span>${nt.den}</div>` +
    `<div class="hz">${Math.round(nt.hz)} Hz</div>` +
    `<div class="cents">${cents}¢${oct}</div>`;
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
function showResult(data) {
  current = data;
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
  document.getElementById("meta").textContent =
    `${data.bars} bars · ${(data.seconds || 0).toFixed(1)}s · 92 bpm`;
  document.getElementById("intro").classList.add("hidden");
  document.getElementById("result").classList.remove("hidden");
}
function reset() {
  stopAudio();
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
  if (playing) return stopAudio();
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
  node.start();
  playing = true;
  btn.textContent = "◼ stop";
}

// ---- beat builder (sparse↔busy) ----
let beatNode = null;
let beatPlaying = false;

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
  if (invoke) return invoke("beat", { busy });
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
  showLanes(data.voices);
  audioCtx = audioCtx || new (window.AudioContext || window.webkitAudioContext)();
  await audioCtx.resume();
  const buf = audioCtx.createBuffer(1, data.samples.length, data.sampleRate);
  buf.copyToChannel(Float32Array.from(data.samples), 0);
  if (beatNode) { try { beatNode.stop(); } catch (_) {} }
  beatNode = audioCtx.createBufferSource();
  beatNode.buffer = buf;
  beatNode.loop = true;
  beatNode.connect(audioCtx.destination);
  beatNode.start();
  beatPlaying = true;
  document.getElementById("beatBtn").textContent = "◼ beat";
}
async function toggleBeat() {
  if (beatPlaying) return stopBeat();
  await playBeat();
}

// ---- wire ----
document.getElementById("recBtn").addEventListener("click", onRecord);
document.getElementById("demoLink").addEventListener("click", (e) => { e.preventDefault(); demo().then(showResult); });
document.getElementById("playBtn").addEventListener("click", togglePlay);
document.getElementById("redoBtn").addEventListener("click", reset);
document.getElementById("beatBtn").addEventListener("click", toggleBeat);
document.getElementById("busyRng").addEventListener("input", () => { if (beatPlaying) playBeat(); });
