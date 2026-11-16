// Render every commentary line (src/lib/ship/phrases.ts) to an audio clip
// with a neural text-to-speech voice, into public/voice/, with a manifest
// of the clip ids. Run it again whenever the phrases change; clips that
// already exist are kept, clips whose line is gone are deleted.
//
// The voice is Kokoro-82M (Apache-2.0, by hexgrad; ONNX build by
// onnx-community), run locally with the `kokoro-js` package; MP3 encoding
// by `@breezystack/lamejs`. Neither is a dependency of the site: install
// them in any scratch folder and run this script from there:
//
//   mkdir voice-tools && cd voice-tools && npm init -y
//   npm install kokoro-js @breezystack/lamejs
//   node <repo>/web/scripts/make-voice.mjs
//
// The first run downloads the model (about 90 MB) into that folder's
// node_modules cache. Needs Node 23.6+ (it imports a .ts file directly).
// Options: VOICE=af_heart (default), SPEED=1.0.

import { createRequire } from "node:module";
import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, "..", "public", "voice");
const { allPhrases, clipId } = await import(pathToFileURL(join(here, "..", "src", "lib", "ship", "phrases.ts")).href);

// Resolve the tools from the folder the script is run in.
const require = createRequire(join(process.cwd(), "noop.js"));
const { KokoroTTS } = await import(pathToFileURL(require.resolve("kokoro-js")).href);
const lame = await import(pathToFileURL(require.resolve("@breezystack/lamejs")).href);
const Mp3Encoder = lame.Mp3Encoder ?? lame.default?.Mp3Encoder;

const voice = process.env.VOICE ?? "af_heart";
const speed = Number(process.env.SPEED ?? "1.0");
const phrases = allPhrases();
mkdirSync(outDir, { recursive: true });

function toMp3(samples, rate) {
  const pcm = new Int16Array(samples.length);
  // Normalize a little below full scale, with a short fade at both ends (no clicks).
  let peak = 1e-6;
  for (const s of samples) peak = Math.max(peak, Math.abs(s));
  const gain = 0.89 / peak;
  const fade = Math.floor(rate * 0.012);
  for (let i = 0; i < samples.length; i++) {
    const edge = Math.min(1, i / fade, (samples.length - 1 - i) / fade);
    pcm[i] = Math.max(-32768, Math.min(32767, Math.round(samples[i] * gain * edge * 32767)));
  }
  const encoder = new Mp3Encoder(1, rate, 64);
  const chunks = [];
  for (let i = 0; i < pcm.length; i += 1152) {
    const out = encoder.encodeBuffer(pcm.subarray(i, i + 1152));
    if (out.length) chunks.push(Buffer.from(out));
  }
  const end = encoder.flush();
  if (end.length) chunks.push(Buffer.from(end));
  return Buffer.concat(chunks);
}

console.log(`${phrases.length} lines, voice ${voice}, speed ${speed}`);
const tts = await KokoroTTS.from_pretrained("onnx-community/Kokoro-82M-v1.0-ONNX", { dtype: "q8", device: "cpu" });
const ids = [];
let made = 0;
for (const text of phrases) {
  const id = clipId(text);
  ids.push(id);
  const file = join(outDir, `${id}.mp3`);
  if (existsSync(file) && !process.env.FORCE) continue;
  const audio = await tts.generate(text, { voice, speed });
  writeFileSync(file, toMp3(audio.audio, audio.sampling_rate));
  made += 1;
  console.log(`${String(made).padStart(3)}  ${id}`);
}
for (const f of readdirSync(outDir)) {
  if (f.endsWith(".mp3") && !ids.includes(f.slice(0, -4))) rmSync(join(outDir, f));
}
writeFileSync(join(outDir, "manifest.json"), JSON.stringify(ids.sort(), null, 1) + "\n");
console.log(`done: ${made} rendered, ${ids.length} in the manifest`);
