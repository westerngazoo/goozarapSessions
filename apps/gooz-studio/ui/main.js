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

// ---- wire ----
document.getElementById("recBtn").addEventListener("click", onRecord);
document.getElementById("demoLink").addEventListener("click", (e) => { e.preventDefault(); demo().then(showResult); });
document.getElementById("playBtn").addEventListener("click", togglePlay);
document.getElementById("redoBtn").addEventListener("click", reset);
