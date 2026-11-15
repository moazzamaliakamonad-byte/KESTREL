"use client";

import { Line } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { rotate, toScene, type Vec3 } from "@/lib/ship/frames";
import { shipLive } from "@/lib/ship/live";
import { useShip } from "@/lib/ship/store";
import { hash, puffTexture } from "./materials";
import { launchLive } from "./Vehicle";

const COUNT = 260;

interface Puff {
  born: number;
  life: number;
  p: THREE.Vector3;
  v: THREE.Vector3;
  size: number;
  grow: number;
  /** 0 exhaust (grey-white), 1 ground dust, 2 hot wake. */
  kind: number;
}

/**
 * Smoke and dust as camera-facing sprites: the exhaust left behind in the
 * air during the burn, the cloud kicked up where the jet meets the ground,
 * and the glowing wake during entry heating. Visual only: none of it acts
 * on the vehicle.
 */
export function Smoke() {
  const mesh = useRef<THREE.InstancedMesh>(null);
  const map = useMemo(() => puffTexture(), []);
  const puffs = useRef<Puff[]>([]);
  const next = useRef(0);
  const clock = useRef(0);
  const emit = useRef({ trail: 0, ground: 0, wake: 0 });
  const m = useMemo(() => new THREE.Matrix4(), []);
  const scale = useMemo(() => new THREE.Vector3(), []);
  const color = useMemo(() => new THREE.Color(), []);

  const spawn = (puff: Omit<Puff, "born">) => {
    const list = puffs.current;
    const item = { ...puff, born: clock.current };
    if (list.length < COUNT) list.push(item);
    else list[next.current] = item;
    next.current = (next.current + 1) % COUNT;
  };

  useFrame(({ camera }, dtReal) => {
    const st = useShip.getState();
    const s = st.mode === "flying" || st.mode === "done" ? shipLive.snapshot : null;
    const dt = st.view.paused ? 0 : Math.min(dtReal, 0.1) * (st.mode === "launch" ? 1 : st.view.speed);
    clock.current += dt;
    const info = st.info[st.scenario];

    if (s && info && dt > 0 && !s.outcome) {
      const base = toScene(addV(s.position, rotate(s.quat, [0, 0, -info.ship.cm_height])));
      const down = toScene(rotate(s.quat, [0, 0, -1]));
      const vel = toScene(s.velocity);
      const running = s.engines.filter((e) => e === "running").length;
      const power = s.thrust_mag / (info.ship.engine_max * Math.max(1, info.ship.engines));
      // Methane exhaust is nearly clear: only near the ground, where the
      // jet meets damp air and the deluge, does it leave steam behind.
      if (running > 0 && base[1] < 500) {
        emit.current.trail += dt * (6 + 14 * power) * (1 - base[1] / 500);
        while (emit.current.trail >= 1) {
          emit.current.trail -= 1;
          const k = hash(clock.current * 91.7 + emit.current.trail);
          spawn({
            life: 3.5 + 3 * k,
            p: new THREE.Vector3(base[0] + down[0] * 70 + (k - 0.5) * 10, base[1] + down[1] * 70, base[2] + down[2] * 70 + (hash(k * 3) - 0.5) * 10),
            // The gas keeps much of the vehicle's own speed, then the air stops it.
            v: new THREE.Vector3(vel[0] * 0.8 + down[0] * 60 + (k - 0.5) * 20, vel[1] * 0.8 + down[1] * 60, vel[2] * 0.8 + down[2] * 60 + (hash(k * 7) - 0.5) * 20),
            size: 9 + 8 * k,
            grow: 12 + 12 * power,
            kind: 0,
          });
        }
        // Where the jet meets the ground or the pad, a cloud rolls outward.
        const height = base[1];
        if (height < 260 && st.planet === "earth") {
          emit.current.ground += dt * 34 * power * (1 - height / 260);
          while (emit.current.ground >= 1) {
            emit.current.ground -= 1;
            const a = hash(clock.current * 13.3 + emit.current.ground) * Math.PI * 2;
            const speed = 18 + 40 * hash(a * 3.3);
            spawn({
              life: 5 + 4 * hash(a),
              p: new THREE.Vector3(base[0] + Math.cos(a) * 6, 3, base[2] + Math.sin(a) * 6),
              v: new THREE.Vector3(Math.cos(a) * speed, 3 + 5 * hash(a * 9), Math.sin(a) * speed),
              size: 10,
              grow: 16,
              kind: 1,
            });
          }
        }
      }
      // A glowing wake streams off the windward side while the heating is high.
      const heat = THREE.MathUtils.clamp((s.heat_flux - 30_000) / 130_000, 0, 1);
      if (heat > 0) {
        emit.current.wake += dt * 60 * heat;
        const c = toScene(s.position);
        const speed = Math.hypot(...s.velocity) || 1;
        const back = toScene([-s.velocity[0] / speed, -s.velocity[1] / speed, -s.velocity[2] / speed]);
        while (emit.current.wake >= 1) {
          emit.current.wake -= 1;
          const k = hash(clock.current * 57.1 + emit.current.wake);
          spawn({
            life: 0.5 + 0.5 * k,
            p: new THREE.Vector3(c[0] + (k - 0.5) * 16, c[1] + (hash(k * 5) - 0.5) * 16, c[2] + (hash(k * 11) - 0.5) * 16),
            v: new THREE.Vector3(back[0] * speed * 0.9, back[1] * speed * 0.9, back[2] * speed * 0.9),
            size: 9 + 6 * k,
            grow: 30,
            kind: 2,
          });
        }
      }
    }
    // Liftoff: the stack's exhaust floods the pad and rolls outward.
    const ls = launchLive.state;
    if (launchLive.active && dt > 0 && ls.power > 0.05 && (ls.phase === "countdown" || ls.phase === "climb") && ls.height < 500) {
      emit.current.ground += dt * 70 * ls.power * (1 - ls.height / 500);
      while (emit.current.ground >= 1) {
        emit.current.ground -= 1;
        const a = hash(clock.current * 13.3 + emit.current.ground) * Math.PI * 2;
        const speed = 25 + 60 * hash(a * 3.3);
        spawn({
          life: 6 + 5 * hash(a),
          p: new THREE.Vector3(Math.cos(a) * 8, 6 + 14 * hash(a * 5), Math.sin(a) * 8),
          v: new THREE.Vector3(Math.cos(a) * speed, 2 + 9 * hash(a * 9), Math.sin(a) * speed),
          size: 14,
          grow: 22,
          kind: 1,
        });
      }
    }
    if (st.mode === "editing" && puffs.current.length) puffs.current = [];

    const inst = mesh.current;
    if (!inst) return;
    let n = 0;
    for (const p of puffs.current) {
      const age = clock.current - p.born;
      if (age > p.life || age < 0) continue;
      const f = age / p.life;
      if (dt > 0) {
        p.p.addScaledVector(p.v, dt);
        p.v.multiplyScalar(Math.exp(-dt * (p.kind === 2 ? 3 : 1.2)));
        if (p.kind === 1) p.v.y += dt * 1.5;
      }
      const size = p.size + p.grow * age;
      scale.setScalar(size);
      m.compose(p.p, camera.quaternion, scale);
      inst.setMatrixAt(n, m);
      const fade = (1 - f) * Math.min(1, age * 6);
      if (p.kind === 2) color.setRGB(2.4 * fade, 1.0 * fade, 0.45 * fade);
      else if (p.kind === 1) color.setRGB(0.5 * fade, 0.46 * fade, 0.4 * fade);
      else color.setRGB(0.32 * fade, 0.32 * fade, 0.34 * fade);
      inst.setColorAt(n, color);
      n += 1;
    }
    inst.count = n;
    inst.instanceMatrix.needsUpdate = true;
    if (inst.instanceColor) inst.instanceColor.needsUpdate = true;
  });

  return (
    <instancedMesh ref={mesh} args={[undefined, undefined, COUNT]} frustumCulled={false} renderOrder={5}>
      <planeGeometry args={[1, 1]} />
      {/* Premultiplied "screen" blending: brightness carries the fade, so one pass serves smoke, dust and glow. */}
      <meshBasicMaterial
        map={map}
        transparent
        premultipliedAlpha
        depthWrite={false}
        blending={THREE.CustomBlending}
        blendSrc={THREE.OneFactor}
        blendDst={THREE.OneMinusSrcColorFactor}
        fog={false}
      />
    </instancedMesh>
  );
}

function addV(a: Vec3, b: Vec3): Vec3 {
  return [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
}

const scene = (pts: Vec3[]) => pts.map((p) => toScene(p));

/** Preview (where this start leads), the current SCvx plan, and the flown trail. */
export function Paths() {
  const preview = useShip((s) => s.preview);
  const mode = useShip((s) => s.mode);
  const show = useShip((s) => s.view);
  const [plan, setPlan] = useState<Vec3[] | null>(null);
  const [trail, setTrail] = useState<Vec3[]>([]);
  const version = useRef(-1);
  const trailLen = useRef(0);

  useFrame(() => {
    if (shipLive.planVersion !== version.current) {
      version.current = shipLive.planVersion;
      setPlan(shipLive.plan ? scene(shipLive.plan.position) : null);
    }
    // Redraw the trail every few points (not every frame).
    if (shipLive.trail.length >= trailLen.current + 4 || shipLive.trail.length < trailLen.current) {
      trailLen.current = shipLive.trail.length;
      setTrail(scene(shipLive.trail));
    }
  });

  const previewPts = useMemo(() => (preview?.ok && preview.path.length > 1 ? scene(preview.path) : null), [preview]);
  const analysis = show.analysis;
  const flown = mode === "flying" || mode === "done";
  return (
    <group>
      {previewPts && mode === "editing" && show.preview && (
        <Line points={previewPts} color="#8fe9ff" lineWidth={2} dashed dashSize={60} gapSize={40} transparent opacity={0.85} fog={false} />
      )}
      {plan && plan.length > 1 && flown && (show.plan || analysis) && (
        <Line points={plan} color={analysis ? "#bff3ff" : "#ffffff"} lineWidth={analysis ? 3 : 2} transparent opacity={0.9} fog={false} />
      )}
      {trail.length > 1 && flown && show.trail && (
        <Line points={trail} color="#ffb35c" lineWidth={1.5} transparent opacity={0.6} fog={false} />
      )}
    </group>
  );
}
