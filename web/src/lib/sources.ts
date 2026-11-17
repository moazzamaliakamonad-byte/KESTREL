/**
 * Where each vehicle number comes from: shown when a visitor points at a
 * "published" or "illustrative" tag. Published = stated by SpaceX or in
 * the public record (as summarized on Wikipedia, "SpaceX Raptor" and
 * "SpaceX Starship"); illustrative = not published, chosen to be
 * plausible, with the reason. Checked 2026-11-17.
 */

export interface Source {
  title: string;
  text: string;
}

export const SOURCES: Record<string, Source> = {
  size: { title: "Published", text: "Ship 52.1 m (Block 2), booster 72.3 m (Block 3), both 9 m in diameter, as reported by SpaceX. Later blocks are reported longer; the figures disagree between sources, so these are kept." },
  engines: { title: "Published", text: "33 Raptors on the booster, the inner 13 gimballing; 3 sea-level and 3 vacuum Raptors on the ship; 4 flaps; 3 grid fins on Block 3." },
  thrust: { title: "Published", text: "Raptor 3: 250 tf (2.45 MN) at sea level in the public record. Some reports of SpaceX's 2024 reveal give 280 tf (2.75 MN); the figures disagree, so the lower is kept." },
  isp: { title: "Published", text: "Sea-level Raptor, about 327 s in the public record (330 s is used for the booster). Raptor 3 is reported at up to 350 s; sea-level and vacuum figures are not always kept apart." },
  throttle: { title: "Published", text: "Raptor throttles from 40 % to 100 % of full thrust. This is why one engine can hover a nearly empty ship but three cannot." },
  dryMass: { title: "Illustrative", text: "SpaceX has not published dry masses. 100 t (ship) and 275 t (booster) are in the range of widely reported estimates." },
  propellant: { title: "Illustrative", text: "How much propellant is left for landing is not published. With 40 t the ship's nominal test flight lands with about 19 t left; with 120 t the booster's with about 35 t (measured, LIMITATIONS.md)." },
  gimbal: { title: "Illustrative", text: "Raptor's gimbal range is not published; one SpaceX test showed 15°, used as the limit here." },
  aero: { title: "Illustrative", text: "Drag of a cylinder across the flow, a slender-body lift slope, flaps as drag plates, grid fins as a force per degree. Not wind-tunnel or flight data." },
  ignition: { title: "Illustrative", text: "Time from the ignition command to thrust. Not published; 1 s is a plausible spin-up for a large staged-combustion engine." },
  lag: { title: "Illustrative", text: "How slowly thrust follows a throttle command (first-order lag). Not published." },
};
