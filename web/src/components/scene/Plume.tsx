"use client";

import { useFrame } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import { live } from "@/lib/live";

const MIN_LENGTH = 6;
const MAX_LENGTH = 28;

const vertex = /* glsl */ `
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`;

// The cone is flipped to point down, so uv.y = 0 at the nozzle and 1 at the
// tip; `along` is 1 at the nozzle. Turbulent noise streaks along the flame,
// hot white-blue core fading to orange at the edges and tip.
const fragment = /* glsl */ `
  uniform float uTime;
  uniform float uIntensity;
  uniform vec3 uCore;
  uniform vec3 uEdge;
  uniform float uSoft;
  varying vec2 vUv;

  float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
  float noise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
  }

  void main() {
    float along = 1.0 - vUv.y;
    float around = abs(fract(vUv.x * 2.0) - 0.5) * 2.0;
    float n = noise(vec2(vUv.x * 18.0, along * 6.0 + uTime * 9.0));
    float streak = 0.65 + 0.35 * n;
    float body = pow(along, 1.6) * streak;
    float edge = smoothstep(0.0, uSoft, 1.0 - around);
    vec3 col = mix(uEdge, uCore, pow(along, 3.0));
    float a = body * edge * uIntensity;
    gl_FragColor = vec4(col * a * 3.0, a);
  }
`;

function flameMaterial(core: string, edge: string, soft: number) {
  return new THREE.ShaderMaterial({
    vertexShader: vertex,
    fragmentShader: fragment,
    uniforms: {
      uTime: { value: 0 },
      uIntensity: { value: 0 },
      uCore: { value: new THREE.Color(core) },
      uEdge: { value: new THREE.Color(edge) },
      uSoft: { value: soft },
    },
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.DoubleSide,
    toneMapped: false,
  });
}

/** Engine flame below the nozzle; length and brightness follow thrust. */
export function Plume() {
  const group = useRef<THREE.Group>(null);
  const light = useRef<THREE.PointLight>(null);
  const core = useMemo(() => flameMaterial("#dfeeff", "#6fb4ff", 0.6), []);
  const outer = useMemo(() => flameMaterial("#ffd28a", "#ff5a1f", 0.9), []);
  // Mutated per frame through refs (React Compiler: memoised values are immutable).
  const coreRef = useRef<THREE.ShaderMaterial>(core);
  const outerRef = useRef<THREE.ShaderMaterial>(outer);

  useFrame(({ clock }) => {
    const s = live.sample;
    const level = s ? Math.min(s.thrustMag / live.maxThrust, 1) : 0;
    const flicker = 0.92 + 0.08 * Math.sin(clock.elapsedTime * 47) * Math.sin(clock.elapsedTime * 31);
    const length = level > 0 ? MIN_LENGTH + (MAX_LENGTH - MIN_LENGTH) * level : 0;
    if (group.current) {
      group.current.visible = level > 0.01;
      group.current.scale.set(1, length * flicker, 1);
    }
    for (const m of [coreRef.current, outerRef.current]) {
      m.uniforms.uTime.value = clock.elapsedTime;
      m.uniforms.uIntensity.value = level * flicker;
    }
    if (light.current) light.current.intensity = level * flicker * 4000;
  });

  return (
    <group position-y={0.2}>
      {/* Unit-height cones pointing down; scaled to the flame length. */}
      <group ref={group}>
        <mesh material={outer} position-y={-0.5} rotation-x={Math.PI}>
          <cylinderGeometry args={[0.05, 1.5, 1, 32, 1, true]} />
        </mesh>
        <mesh material={core} position-y={-0.5} rotation-x={Math.PI} scale={[0.55, 0.7, 0.55]}>
          <cylinderGeometry args={[0.05, 1.4, 1, 32, 1, true]} />
        </mesh>
      </group>
      <pointLight ref={light} color="#ff9a52" distance={160} decay={1.4} position-y={-3} />
    </group>
  );
}
