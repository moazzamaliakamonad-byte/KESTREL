"use client";

import { useFrame } from "@react-three/fiber";
import { Bloom, EffectComposer, SMAA, ToneMapping, Vignette } from "@react-three/postprocessing";
import { BlendFunction, Effect, ToneMappingMode } from "postprocessing";
import { useMemo, useRef } from "react";
import { Uniform } from "three";
import { useShip } from "@/lib/ship/store";

export type Quality = "high" | "low";

const fragment = /* glsl */ `
  uniform float uProgress; // 0 = film look, 1 = blueprint
  uniform float uTime;

  float lum(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }

  void mainImage(const in vec4 inputColor, const in vec2 uv, out vec4 outputColor) {
    if (uProgress <= 0.0005) { outputColor = inputColor; return; }
    // The change spreads from the centre as a shockwave: a ring that bends
    // the picture as it passes and leaves the blueprint behind it.
    vec2 c = (uv - 0.5) * vec2(aspect, 1.0);
    float r = length(c);
    float front = uProgress * 1.35;
    float d = r - front;
    float moving = step(uProgress, 0.999);
    float ripple = exp(-d * d * 260.0) * moving;
    vec2 dir = r > 1e-4 ? c / r : vec2(0.0);
    // Refraction at the front, and a slight twist behind it that settles.
    vec2 bent = uv - dir * ripple * 0.035 / vec2(aspect, 1.0);
    float twist = (1.0 - uProgress) * smoothstep(0.0, -0.35, d) * 0.12 * moving;
    vec2 q = (bent - 0.5) * vec2(aspect, 1.0);
    q = mat2(cos(twist), -sin(twist), sin(twist), cos(twist)) * q;
    bent = q / vec2(aspect, 1.0) + 0.5;
    vec4 src = texture2D(inputBuffer, bent);
    // Colour fringes on the ring.
    src.r = mix(src.r, texture2D(inputBuffer, bent + dir * ripple * 0.012).r, moving);
    src.b = mix(src.b, texture2D(inputBuffer, bent - dir * ripple * 0.012).b, moving);
    float revealed = smoothstep(0.012, -0.012, d);

    float l = lum(src.rgb);
    // Edges from the picture itself (four taps).
    float lx = lum(texture2D(inputBuffer, bent + vec2(texelSize.x, 0.0)).rgb) - lum(texture2D(inputBuffer, bent - vec2(texelSize.x, 0.0)).rgb);
    float ly = lum(texture2D(inputBuffer, bent + vec2(0.0, texelSize.y)).rgb) - lum(texture2D(inputBuffer, bent - vec2(0.0, texelSize.y)).rgb);
    float edge = clamp(length(vec2(lx, ly)) * 9.0, 0.0, 1.0);

    vec3 deep = vec3(0.012, 0.045, 0.13);
    vec3 mid = vec3(0.05, 0.3, 0.62);
    vec3 bright = vec3(0.55, 0.9, 1.0);
    vec3 blue = mix(deep, mid, smoothstep(0.0, 0.8, l));
    blue = mix(blue, bright, smoothstep(0.8, 1.0, l));
    blue += edge * vec3(0.35, 0.8, 1.0);

    // Drafting grid, fine and coarse.
    vec2 g = uv * vec2(aspect, 1.0);
    vec2 fine = abs(fract(g * 40.0) - 0.5);
    vec2 coarse = abs(fract(g * 8.0) - 0.5);
    float grid = smoothstep(0.47, 0.5, max(fine.x, fine.y)) * 0.05 + smoothstep(0.485, 0.5, max(coarse.x, coarse.y)) * 0.1;
    blue += grid * vec3(0.3, 0.7, 1.0);
    // Faint moving scan lines.
    blue *= 0.94 + 0.06 * sin(uv.y * resolution.y * 1.2 + uTime * 6.0);

    // Strongly coloured warm things (the force arrows) keep their colour.
    float hi = max(src.r, max(src.g, src.b));
    float lo = min(src.r, min(src.g, src.b));
    float keep = smoothstep(0.38, 0.6, (hi - lo) / max(hi, 1e-3)) * step(src.b, src.r) * smoothstep(0.35, 0.6, hi);
    blue = mix(blue, src.rgb, keep);
    vec3 col = mix(src.rgb, blue, revealed);
    // The ring itself, and a second fainter one chasing it.
    col += ripple * vec3(0.55, 0.95, 1.0) * 1.6;
    float d2 = r - front * 0.72;
    col += exp(-d2 * d2 * 900.0) * moving * vec3(0.3, 0.7, 1.0) * 0.6;
    outputColor = vec4(col, inputColor.a);
  }
`;

/** Turns the finished picture into a blueprint: blue ramp, glowing edges, drafting grid. */
class BlueprintEffect extends Effect {
  constructor() {
    super("BlueprintEffect", fragment, {
      blendFunction: BlendFunction.NORMAL,
      uniforms: new Map<string, Uniform>([
        ["uProgress", new Uniform(0)],
        ["uTime", new Uniform(0)],
      ]),
    });
  }
}

/**
 * The finish: glow on hot things, film tone mapping, vignette, and the
 * analysis view's blueprint look, which sweeps in over about a second.
 */
export function Post({ quality }: { quality: Quality }) {
  const blueprint = useMemo(() => new BlueprintEffect(), []);
  const progress = useRef(0);
  useFrame(({ clock }, dt) => {
    const st = useShip.getState();
    const target = st.view.analysis && (st.mode === "flying" || st.mode === "done") ? 1 : 0;
    const rate = target ? 0.85 : 2.0;
    progress.current = Math.min(1, Math.max(0, progress.current + Math.sign(target - progress.current) * rate * Math.min(dt, 0.05)));
    // Ease out so the line slows as it finishes.
    const p = progress.current;
    (blueprint.uniforms.get("uProgress") as Uniform).value = 1 - (1 - p) * (1 - p);
    (blueprint.uniforms.get("uTime") as Uniform).value = clock.elapsedTime;
  });
  if (quality === "low") {
    return (
      <EffectComposer multisampling={0}>
        <ToneMapping mode={ToneMappingMode.ACES_FILMIC} />
        <primitive object={blueprint} dispose={null} />
        <SMAA />
      </EffectComposer>
    );
  }
  return (
    <EffectComposer multisampling={0}>
      <Bloom mipmapBlur luminanceThreshold={0.92} intensity={1.35} radius={0.75} />
      <ToneMapping mode={ToneMappingMode.ACES_FILMIC} />
      <primitive object={blueprint} dispose={null} />
      <Vignette offset={0.28} darkness={0.62} />
      <SMAA />
    </EffectComposer>
  );
}
