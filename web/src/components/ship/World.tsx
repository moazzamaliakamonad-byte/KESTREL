"use client";

import { useMemo } from "react";
import * as THREE from "three";
import { Terrain } from "@/components/scene/Terrain";
import { useShip } from "@/lib/ship/store";

/** Gradient sky dome: Earth daylight or Mars dusk (no textures). */
function SkyDome({ planet }: { planet: "earth" | "mars" }) {
  const material = useMemo(() => {
    const top = planet === "earth" ? new THREE.Color("#2f6fb8") : new THREE.Color("#3a221a");
    const horizon = planet === "earth" ? new THREE.Color("#cfe3f2") : new THREE.Color("#c49a76");
    return new THREE.ShaderMaterial({
      side: THREE.BackSide,
      depthWrite: false,
      uniforms: { uTop: { value: top }, uHorizon: { value: horizon } },
      vertexShader: /* glsl */ `
        varying vec3 vDir;
        void main() {
          vDir = normalize(position);
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
          gl_Position.z = gl_Position.w;
        }`,
      fragmentShader: /* glsl */ `
        uniform vec3 uTop; uniform vec3 uHorizon; varying vec3 vDir;
        void main() {
          float h = clamp(vDir.y, 0.0, 1.0);
          gl_FragColor = vec4(mix(uHorizon, uTop, pow(h, 0.45)), 1.0);
        }`,
    });
  }, [planet]);
  return (
    <mesh material={material} renderOrder={-1}>
      <sphereGeometry args={[18000, 32, 16]} />
    </mesh>
  );
}

/** Coastal ground for Earth: sand, a concrete pad, sea to the south. */
function Coast() {
  const sand = useMemo(() => {
    const c = document.createElement("canvas");
    c.width = c.height = 256;
    const g = c.getContext("2d")!;
    const img = g.createImageData(256, 256);
    for (let i = 0; i < img.data.length; i += 4) {
      // Deterministic grain (a hash, not Math.random: render must be pure).
      const k = i / 4;
      const n = 200 + 30 * ((Math.sin(k * 12.9898 + (k % 256) * 78.233) * 43758.5453) % 1 + 1) % 1;
      img.data[i] = n;
      img.data[i + 1] = n * 0.9;
      img.data[i + 2] = n * 0.72;
      img.data[i + 3] = 255;
    }
    g.putImageData(img, 0, 0);
    const t = new THREE.CanvasTexture(c);
    t.wrapS = t.wrapT = THREE.RepeatWrapping;
    t.repeat.set(200, 200);
    t.colorSpace = THREE.SRGBColorSpace;
    return t;
  }, []);
  return (
    <group>
      <mesh rotation-x={-Math.PI / 2} position={[0, 0, -4000]} receiveShadow>
        <planeGeometry args={[30000, 10000]} />
        <meshStandardMaterial map={sand} roughness={1} />
      </mesh>
      <mesh rotation-x={-Math.PI / 2} position={[0, -0.5, 6000]}>
        <planeGeometry args={[30000, 10000]} />
        <meshStandardMaterial color="#1f5f7a" metalness={0.2} roughness={0.15} />
      </mesh>
      <mesh rotation-x={-Math.PI / 2} position={[-10, 0.05, 0]} receiveShadow>
        <planeGeometry args={[120, 90]} />
        <meshStandardMaterial color="#a7a39c" roughness={0.95} />
      </mesh>
    </group>
  );
}

/** Mars: the procedural terrain from M1 and a landing-pad ring. */
function MarsGround() {
  return (
    <group>
      <Terrain />
      <mesh rotation-x={-Math.PI / 2} position-y={0.3}>
        <ringGeometry args={[14, 16, 64]} />
        <meshBasicMaterial color="#7fe3ff" transparent opacity={0.6} />
      </mesh>
    </group>
  );
}

export function World() {
  const scenario = useShip((s) => s.scenario);
  return (
    <group>
      <SkyDome planet={scenario} />
      {scenario === "earth" ? <Coast /> : <MarsGround />}
    </group>
  );
}
