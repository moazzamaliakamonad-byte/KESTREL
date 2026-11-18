"use client";

import { useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { gridTexture, hash } from "./materials";

/** Height of the launch mount's table, m. */
export const MOUNT_TOP = 22.5;

/** Concrete apron: slabs with joints, tyre-dark stains, scorching around the mount, painted lines. */
function apronTexture(): THREE.Texture {
  const n = 1024;
  const c = document.createElement("canvas");
  c.width = c.height = n;
  const g = c.getContext("2d")!;
  g.fillStyle = "#8f8b84";
  g.fillRect(0, 0, n, n);
  // Slabs, each a slightly different grey.
  const slab = n / 16;
  for (let y = 0; y < 16; y++) {
    for (let x = 0; x < 16; x++) {
      const v = 128 + Math.floor(26 * hash(x * 17 + y * 131));
      g.fillStyle = `rgb(${v + 8}, ${v + 5}, ${v})`;
      g.fillRect(x * slab + 1, y * slab + 1, slab - 2, slab - 2);
    }
  }
  // Grain.
  for (let i = 0; i < 9000; i++) {
    g.fillStyle = `rgba(${hash(i) > 0.5 ? 255 : 0}, ${hash(i) > 0.5 ? 255 : 0}, ${hash(i) > 0.5 ? 255 : 0}, 0.035)`;
    g.fillRect(hash(i * 1.7) * n, hash(i * 3.1) * n, 2 + 3 * hash(i * 5.3), 2 + 3 * hash(i * 7.9));
  }
  // Scorch and water staining spreading from the mount (centre-right of the apron).
  const [cx, cy] = [n * 0.58, n * 0.5];
  const burn = g.createRadialGradient(cx, cy, n * 0.03, cx, cy, n * 0.34);
  burn.addColorStop(0, "rgba(20, 18, 16, 0.85)");
  burn.addColorStop(0.35, "rgba(40, 34, 30, 0.45)");
  burn.addColorStop(1, "rgba(60, 54, 48, 0)");
  g.fillStyle = burn;
  g.fillRect(0, 0, n, n);
  g.strokeStyle = "rgba(30, 26, 22, 0.22)";
  for (let i = 0; i < 46; i++) {
    const a = hash(i * 9.1) * Math.PI * 2;
    g.lineWidth = 2 + 8 * hash(i * 2.3);
    g.beginPath();
    g.moveTo(cx + Math.cos(a) * n * 0.05, cy + Math.sin(a) * n * 0.05);
    g.lineTo(cx + Math.cos(a) * n * (0.16 + 0.22 * hash(i)), cy + Math.sin(a) * n * (0.16 + 0.22 * hash(i)));
    g.stroke();
  }
  // Painted keep-out ring and edge lines.
  g.strokeStyle = "rgba(226, 184, 60, 0.85)";
  g.lineWidth = 5;
  g.beginPath();
  g.arc(cx, cy, n * 0.2, 0, Math.PI * 2);
  g.stroke();
  g.setLineDash([26, 18]);
  g.strokeRect(14, 14, n - 28, n - 28);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = 8;
  return t;
}

/** A vertical storage tank with a domed top and a dark band. */
function Tank({ x, z, r, h, color = "#d9dde1" }: { x: number; z: number; r: number; h: number; color?: string }) {
  return (
    <group position={[x, 0, z]}>
      <mesh position-y={h / 2} castShadow receiveShadow>
        <cylinderGeometry args={[r, r, h, 28]} />
        <meshStandardMaterial color={color} metalness={0.35} roughness={0.5} />
      </mesh>
      <mesh position-y={h} scale={[1, 0.45, 1]} castShadow>
        <sphereGeometry args={[r, 24, 12, 0, Math.PI * 2, 0, Math.PI / 2]} />
        <meshStandardMaterial color={color} metalness={0.35} roughness={0.5} />
      </mesh>
      <mesh position-y={h * 0.18}>
        <cylinderGeometry args={[r + 0.06, r + 0.06, 1.4, 28, 1, true]} />
        <meshStandardMaterial color="#3a3e44" roughness={0.8} />
      </mesh>
    </group>
  );
}

/** A flood-light mast: a thin pole with a bank of lamps on top. */
function Mast({ x, z, h = 62 }: { x: number; z: number; h?: number }) {
  return (
    <group position={[x, 0, z]}>
      <mesh position-y={h / 2} castShadow>
        <cylinderGeometry args={[0.35, 0.9, h, 8]} />
        <meshStandardMaterial color="#5b5f66" metalness={0.6} roughness={0.5} />
      </mesh>
      <mesh position-y={h + 1.2} rotation-y={Math.atan2(-x, -z)}>
        <boxGeometry args={[7, 2.4, 0.8]} />
        <meshStandardMaterial color="#22252a" emissive="#fff3d0" emissiveIntensity={0.9} />
      </mesh>
    </group>
  );
}

/** Low coastal scrub scattered over the sand: many small, flat clumps (large ones read as green boulders). */
function Scrub() {
  const ref = useRef<THREE.InstancedMesh>(null);
  const count = 1500;
  useLayoutEffect(() => {
    const m = ref.current;
    if (!m) return;
    const mat = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const tint = new THREE.Color();
    let n = 0;
    for (let i = 0; n < count && i < count * 4; i++) {
      const a = hash(i * 1.3) * Math.PI * 2;
      const d = 190 + 2300 * hash(i * 2.9) ** 1.6;
      const [x, z] = [Math.cos(a) * d - 60, Math.sin(a) * d - 200];
      // Keep the road, the tank farm and the shore clear.
      if (z > 520 || (Math.abs(z + 40) < 14 && x < -90) || (x < -110 && x > -330 && z < -40 && z > -170)) continue;
      const s = 0.9 + 2.6 * hash(i * 4.7) ** 2;
      mat.compose(new THREE.Vector3(x, s * 0.1, z), q, new THREE.Vector3(s, s * 0.3, s * (0.7 + 0.6 * hash(i))));
      m.setColorAt(n, tint.setHSL(0.2 + 0.06 * hash(i * 8.3), 0.3 + 0.12 * hash(i * 6.1), 0.11 + 0.06 * hash(i * 3.7)));
      m.setMatrixAt(n++, mat);
    }
    m.count = n;
    m.instanceMatrix.needsUpdate = true;
    if (m.instanceColor) m.instanceColor.needsUpdate = true;
  }, []);
  return (
    <instancedMesh ref={ref} args={[undefined, undefined, count]} receiveShadow>
      <sphereGeometry args={[1, 8, 6]} />
      <meshStandardMaterial color="#ffffff" roughness={1} />
    </instancedMesh>
  );
}

/**
 * The launch site: concrete apron, the launch mount under the catch axis
 * (legs, ring table, hold-down clamps, deluge plate), propellant tank
 * farm with its pipe run, flood-light masts, a control building, a road
 * and berm, scrub, and a second tower in the distance. Generic shapes at
 * about real scale; nothing here is anyone's drawing.
 */
export function LaunchSite() {
  const apron = useMemo(() => apronTexture(), []);
  const lattice = useMemo(() => {
    const t = gridTexture();
    t.repeat.set(2, 18);
    return t;
  }, []);
  const clamps = useMemo(() => Array.from({ length: 20 }, (_, i) => (i / 20) * Math.PI * 2), []);
  return (
    <group>
      {/* Apron. */}
      <mesh rotation-x={-Math.PI / 2} position={[-18, 0.06, 0]} receiveShadow>
        <planeGeometry args={[210, 150]} />
        <meshStandardMaterial map={apron} roughness={0.92} />
      </mesh>
      {/* Access road with a centre line, and a berm between pad and tank farm. */}
      <mesh rotation-x={-Math.PI / 2} position={[-420, 0.04, -40]} receiveShadow>
        <planeGeometry args={[600, 13]} />
        <meshStandardMaterial color="#5f5d5a" roughness={0.95} />
      </mesh>
      <mesh rotation-x={-Math.PI / 2} position={[-420, 0.07, -40]}>
        <planeGeometry args={[600, 0.35]} />
        <meshStandardMaterial color="#d8d2bd" roughness={0.9} />
      </mesh>
      <mesh position={[-108, 4.5, -95]} rotation-y={0.12} castShadow receiveShadow>
        <cylinderGeometry args={[4, 13, 9, 4, 1]} />
        <meshStandardMaterial color="#a39170" roughness={1} />
      </mesh>
      <mesh position={[-108, 4.5, -95]} rotation-y={0.12} scale={[1, 1, 9]} castShadow receiveShadow>
        <boxGeometry args={[14, 9, 14]} />
        <meshStandardMaterial color="#a39170" roughness={1} />
      </mesh>

      {/* Launch mount: legs, ring table with its opening, clamps, deluge plate. */}
      <mesh rotation-x={-Math.PI / 2} position-y={0.12} receiveShadow>
        <circleGeometry args={[13, 48]} />
        <meshStandardMaterial color="#2d2f33" metalness={0.7} roughness={0.55} />
      </mesh>
      {[0, 60, 120, 180, 240, 300].map((d) => {
        const a = THREE.MathUtils.degToRad(d + 30);
        return (
          <group key={d} position={[10 * Math.cos(a), 0, 10 * Math.sin(a)]} rotation-y={-a}>
            <mesh position-y={MOUNT_TOP / 2 - 1.5} castShadow receiveShadow>
              <boxGeometry args={[3.2, MOUNT_TOP - 3, 2.6]} />
              <meshStandardMaterial color="#86837d" roughness={0.9} />
            </mesh>
            <mesh position={[-1.6, 7, 0]} rotation-z={0.35} castShadow>
              <boxGeometry args={[0.9, 15, 0.9]} />
              <meshStandardMaterial color="#6f6c67" roughness={0.9} />
            </mesh>
          </group>
        );
      })}
      <mesh position-y={MOUNT_TOP - 1.7} castShadow receiveShadow>
        <cylinderGeometry args={[12.2, 12.6, 3.4, 48, 1, true]} />
        <meshStandardMaterial color="#6d6a65" roughness={0.85} side={THREE.DoubleSide} />
      </mesh>
      <mesh position-y={MOUNT_TOP - 1.7}>
        <cylinderGeometry args={[4.8, 4.8, 3.4, 48, 1, true]} />
        <meshStandardMaterial color="#3a3835" roughness={0.9} side={THREE.DoubleSide} />
      </mesh>
      <mesh position-y={MOUNT_TOP} rotation-x={-Math.PI / 2} receiveShadow>
        <ringGeometry args={[4.8, 12.2, 48]} />
        <meshStandardMaterial color="#5c5955" roughness={0.9} side={THREE.DoubleSide} />
      </mesh>
      <mesh position-y={MOUNT_TOP - 3.4} rotation-x={-Math.PI / 2}>
        <ringGeometry args={[4.8, 12.6, 48]} />
        <meshStandardMaterial color="#4a4845" roughness={0.9} side={THREE.DoubleSide} />
      </mesh>
      {clamps.map((a) => (
        <mesh key={a} position={[5.4 * Math.cos(a), MOUNT_TOP + 0.5, 5.4 * Math.sin(a)]} rotation-y={-a} castShadow>
          <boxGeometry args={[1.5, 1, 0.7]} />
          <meshStandardMaterial color="#2f3136" metalness={0.7} roughness={0.5} />
        </mesh>
      ))}
      {/* Handrail around the table. */}
      <mesh position-y={MOUNT_TOP + 1.1}>
        <torusGeometry args={[12, 0.08, 6, 64]} onUpdate={(g) => g.rotateX(Math.PI / 2)} />
        <meshStandardMaterial color="#c9a233" roughness={0.7} />
      </mesh>
      {/* Pipe run from the mount to the tank farm. */}
      {[0, 1.4, 2.8].map((o) => (
        <mesh key={o} position={[-85, 1.2 + o * 0.5, -58 - o]} rotation-z={Math.PI / 2} castShadow>
          <cylinderGeometry args={[0.45, 0.45, 150, 10]} />
          <meshStandardMaterial color={o === 0 ? "#c7ccd1" : "#8f959c"} metalness={0.6} roughness={0.45} />
        </mesh>
      ))}

      {/* Tank farm: tall cryogenic tanks, water tanks, horizontal tanks on saddles. */}
      {[0, 1, 2, 3, 4].map((i) => (
        <Tank key={`v${i}`} x={-170 - i * 24} z={-78} r={6} h={30 + (i % 2) * 6} />
      ))}
      {[0, 1, 2].map((i) => (
        <Tank key={`w${i}`} x={-182 - i * 30} z={-116} r={9} h={16} color="#c2c6ca" />
      ))}
      {[0, 1, 2, 3].map((i) => (
        <group key={`h${i}`} position={[-300, 4.2, -70 - i * 13]}>
          <mesh rotation-z={Math.PI / 2} castShadow>
            <capsuleGeometry args={[3.2, 24, 8, 20]} />
            <meshStandardMaterial color="#dfe3e6" metalness={0.35} roughness={0.45} />
          </mesh>
          {[-8, 8].map((sx) => (
            <mesh key={sx} position={[sx, -2.8, 0]}>
              <boxGeometry args={[1.2, 2.8, 5]} />
              <meshStandardMaterial color="#77736d" roughness={0.9} />
            </mesh>
          ))}
        </group>
      ))}

      {/* Control building and flood-light masts. */}
      <group position={[-250, 0, 60]}>
        <mesh position-y={5} castShadow receiveShadow>
          <boxGeometry args={[46, 10, 22]} />
          <meshStandardMaterial color="#b9bcc0" roughness={0.8} />
        </mesh>
        <mesh position={[0, 6, 11.1]}>
          <planeGeometry args={[40, 2.2]} />
          <meshStandardMaterial color="#16202a" metalness={0.8} roughness={0.2} />
        </mesh>
        <mesh position={[10, 11.2, 0]}>
          <boxGeometry args={[8, 2.4, 6]} />
          <meshStandardMaterial color="#8a8d92" roughness={0.8} />
        </mesh>
      </group>
      <Mast x={95} z={-78} />
      <Mast x={95} z={78} />
      <Mast x={-120} z={82} />
      <Mast x={-128} z={-20} h={54} />
      <Scrub />

      {/* A second launch-and-catch tower, two kilometres down the coast. */}
      <group position={[-1900, 0, 260]}>
        <mesh position-y={72}>
          <boxGeometry args={[11, 144, 11]} />
          <meshStandardMaterial map={lattice} alphaTest={0.45} color="#3c4046" metalness={0.6} roughness={0.6} side={THREE.DoubleSide} />
        </mesh>
        <mesh position={[14, 96, 0]}>
          <boxGeometry args={[30, 3, 9]} />
          <meshStandardMaterial color="#3c4046" roughness={0.7} />
        </mesh>
      </group>
    </group>
  );
}
