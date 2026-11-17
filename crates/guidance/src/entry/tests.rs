use super::*;
use model::entry::{EntryAero, surface_point};

/// Fly a guided entry on the model itself (truth = model, bank rolling at
/// 15°/s): (hand-over state, peak tile temperature, reversals, seconds).
fn fly(range: f64, gamma_deg: f64, crossrange: f64) -> (EntryState, f64, u32, f64, EntryTarget) {
    let aero = EntryAero::ship(52.0, 4.5);
    let m = 140e3;
    let site = surface_point(26f64.to_radians(), (-97.15f64).to_radians());
    let (mut x, mut target) = entry_start(
        site,
        range,
        7_500.0,
        gamma_deg.to_radians(),
        25_000.0,
        25_000.0,
    );
    // Shift the aim sideways (crossrange) to exercise the bank reversals.
    if crossrange != 0.0 {
        let north = unit(quat::sub(
            [0.0, 0.0, 1.0],
            quat::scale(target.aim, target.aim[2]),
        ));
        target.aim = unit(quat::add(
            target.aim,
            quat::scale(north, crossrange / entry::RADIUS),
        ));
    }
    let mut g = EntryGuidance::new(target);
    let (mut bank, mut t, mut peak, mut since) = (0.0f64, 0.0, 0.0f64, f64::INFINITY);
    let mut cmd = 0.0;
    while entry::altitude(&x) > 25_000.0 && t < 4_000.0 {
        if since >= CYCLE {
            cmd = g.command(&aero, &x, m).bank;
            since = 0.0;
        }
        let dt = 0.1;
        bank += (cmd - bank).clamp(-0.26 * dt, 0.26 * dt);
        x = entry::rk4(&aero, &x, m, bank, dt);
        t += dt;
        since += dt;
        let q = entry::heat_flux(
            entry::density(entry::altitude(&x)),
            quat::norm(entry::velocity(&x)),
        );
        peak = peak.max(entry::tile_temperature(q));
    }
    (x, peak, g.reversals, t, target)
}

#[test]
fn guided_entry_reaches_the_aim_point_without_overheating() {
    let (x, peak, reversals, t, target) = fly(3_300e3, -1.0, 0.0);
    let g = EntryGuidance::new(target);
    let to_site = entry::surface_distance(entry::position(&x), target.site);
    let heading = g.heading_error(&x).to_degrees();
    eprintln!(
        "entry: {t:.0} s, {to_site:.0} m from the site, heading error {heading:.2} deg, peak tile {peak:.0} K, {reversals} reversals, v {:.0} m/s, gamma {:.1} deg",
        quat::norm(entry::velocity(&x)),
        entry::flight_path_angle(&x).to_degrees()
    );
    // Hands over 25 km before the site, lined up on it.
    assert!(
        (to_site - 25_000.0).abs() < 5_000.0,
        "{to_site:.0} m from the site"
    );
    assert!(heading.abs() < 3.0, "heading error {heading:.2} deg");
    assert!(peak < entry::TILE_LIMIT, "tiles at {peak:.0} K");
}

/// Exploration: what range and bank give a nominal entry (`--ignored`).
#[test]
#[ignore]
fn entry_sweep() {
    for range in [3_000e3, 3_300e3, 3_600e3] {
        for gamma in [-0.8, -1.0, -1.4] {
            let (x, peak, rev, t, target) = fly(range, gamma, 0.0);
            let miss = entry::surface_distance(entry::position(&x), target.aim);
            eprintln!(
                "range {:.0} km gamma {gamma}: {t:.0} s, miss {miss:.0} m, peak {peak:.0} K, rev {rev}, v {:.0}",
                range / 1e3,
                quat::norm(entry::velocity(&x))
            );
        }
    }
}
