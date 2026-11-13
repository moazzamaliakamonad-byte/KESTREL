"use client";

import { useFrame } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import { live } from "@/lib/live";

const COUNT = 600;

/** Deterministic per-particle pseudo-random in [0, 1). */
const rand = (i: number, k: number) => {
  const x = Math.sin(i * 127.1 + k * 311.7) * 43758.5453;
  return x - Math.floor(x);
};
/** Dust starts below this altitude (m)… */
const START_ALTITUDE = 60;

const vertex = /* glsl */ `
  attribute float aAngle;
  attribute float aPhase;
  attribute float aSpeed;
  uniform float uTime;
  uniform float uStrength;
  uniform float uGroundY;
  varying float vFade;
  void main() {
    float life = fract(aPhase + uTime * aSpeed * 0.35);
    float r = 3.0 + life * 42.0 * aSpeed;
    vec3 p = vec3(cos(aAngle) * r, uGroundY + 0.4 + life * (2.0 + 6.0 * aSpeed), sin(aAngle) * r);
    vFade = (1.0 - life) * uStrength;
    vec4 mv = modelViewMatrix * vec4(p, 1.0);
    gl_Position = projectionMatrix * mv;
    gl_PointSize = (6.0 + 18.0 * life) * (220.0 / -mv.z);
  }
`;

const fragment = /* glsl */ `
  varying float vFade;
  void main() {
    float d = length(gl_PointCoord - 0.5);
    float a = smoothstep(0.5, 0.0, d) * vFade * 0.35;
    gl_FragColor = vec4(vec3(0.62, 0.42, 0.30), a);
  }
`;

/**
 * Dust blown off the ground by the plume near touchdown. Lives in the
 * booster's group but counter-rotates to stay level, and sits on the ground
 * beneath the nozzle.
 */
export function Dust() {
  const points = useRef<THREE.Points>(null);
  const mat = useRef<THREE.ShaderMaterial>(null);
  const geometry = useMemo(() => {
    const g = new THREE.BufferGeometry();
    const angle = new Float32Array(COUNT);
    const phase = new Float32Array(COUNT);
    const speed = new Float32Array(COUNT);
    for (let i = 0; i < COUNT; i++) {
      angle[i] = rand(i, 1) * Math.PI * 2;
      phase[i] = rand(i, 2);
      speed[i] = 0.4 + rand(i, 3) * 0.6;
    }
    g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(COUNT * 3), 3));
    g.setAttribute("aAngle", new THREE.BufferAttribute(angle, 1));
    g.setAttribute("aPhase", new THREE.BufferAttribute(phase, 1));
    g.setAttribute("aSpeed", new THREE.BufferAttribute(speed, 1));
    return g;
  }, []);
  const uniforms = useMemo(
    () => ({ uTime: { value: 0 }, uStrength: { value: 0 }, uGroundY: { value: 0 } }),
    [],
  );

  useFrame(({ clock }) => {
    const s = live.sample;
    const p = points.current;
    const m = mat.current;
    if (!s || !p || !p.parent || !m) return;
    const altitude = s.position[2];
    const level = s.thrustMag / live.maxThrust;
    const near = 1 - Math.min(Math.max(altitude / START_ALTITUDE, 0), 1);
    m.uniforms.uStrength.value = level * near;
    m.uniforms.uTime.value = clock.elapsedTime;
    m.uniforms.uGroundY.value = -altitude;
    // Undo the booster's tilt so the dust ring lies flat on the ground.
    p.quaternion.copy(p.parent.quaternion).invert();
    p.visible = near > 0 && level > 0.01;
  });

  return (
    <points ref={points} geometry={geometry} frustumCulled={false}>
      <shaderMaterial
        ref={mat}
        vertexShader={vertex}
        fragmentShader={fragment}
        uniforms={uniforms}
        transparent
        depthWrite={false}
      />
    </points>
  );
}
