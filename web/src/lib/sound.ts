/**
 * Engine rumble synthesised with WebAudio: brown noise through a low-pass
 * filter whose cutoff and gain follow the thrust level. No audio files.
 */
export function createRumble(): { setLevel(x: number): void; dispose(): void } {
  const ctx = new AudioContext();
  const seconds = 2;
  const buffer = ctx.createBuffer(1, ctx.sampleRate * seconds, ctx.sampleRate);
  const data = buffer.getChannelData(0);
  let last = 0;
  for (let i = 0; i < data.length; i++) {
    // Brown noise: integrated white noise, kept bounded.
    const white = Math.random() * 2 - 1;
    last = (last + 0.02 * white) / 1.02;
    data[i] = last * 3.5;
  }
  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.loop = true;
  const filter = ctx.createBiquadFilter();
  filter.type = "lowpass";
  filter.frequency.value = 80;
  const gain = ctx.createGain();
  gain.gain.value = 0;
  source.connect(filter).connect(gain).connect(ctx.destination);
  source.start();

  return {
    setLevel(x: number) {
      const level = Math.min(Math.max(x, 0), 1);
      const t = ctx.currentTime;
      filter.frequency.setTargetAtTime(80 + 520 * level, t, 0.05);
      gain.gain.setTargetAtTime(0.9 * level, t, 0.05);
    },
    dispose() {
      source.stop();
      void ctx.close();
    },
  };
}
