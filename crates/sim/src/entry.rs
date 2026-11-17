//! Truth for the entry from orbit: the ship as a point mass on a round,
//! non-rotating Earth (`model::entry`), its bank angle rolling toward the
//! commanded one at a limited rate, its tiles heating. Ends when it reaches
//! the hand-over height (the 6-DoF simulator takes over there) or when the
//! heat shield fails. Simulation only.

use model::entry::{self, EntryAero, EntryState};
use model::quat;

/// Fastest roll of the bank angle, rad/s (*illustrative*, 15°/s: the ship
/// rolls with its flaps).
pub const BANK_RATE: f64 = 0.261_799_387_799_149_4;
/// Integration step, s.
pub const ENTRY_STEP: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryOutcome {
    /// Reached the hand-over height.
    HandOver,
    /// The tiles stayed above their limit too long.
    BurnedThrough,
}

#[derive(Debug, Clone)]
pub struct EntrySim {
    pub aero: EntryAero,
    pub mass: f64,
    pub x: EntryState,
    pub t: f64,
    /// Actual and commanded bank, rad.
    pub bank: f64,
    pub bank_cmd: f64,
    /// Integrated heat at the stagnation point, J/m².
    pub heat_load: f64,
    /// Seconds spent above the tile limit.
    pub overheat: f64,
    pub peak_heat_flux: f64,
    pub peak_tile: f64,
    /// Largest aerodynamic deceleration, g.
    pub peak_g: f64,
    pub handover_altitude: f64,
    pub outcome: Option<EntryOutcome>,
}

impl EntrySim {
    pub fn new(aero: EntryAero, mass: f64, x: EntryState, handover_altitude: f64) -> EntrySim {
        EntrySim {
            aero,
            mass,
            x,
            t: 0.0,
            bank: 0.0,
            bank_cmd: 0.0,
            heat_load: 0.0,
            overheat: 0.0,
            peak_heat_flux: 0.0,
            peak_tile: 0.0,
            peak_g: 0.0,
            handover_altitude,
            outcome: None,
        }
    }

    pub fn altitude(&self) -> f64 {
        entry::altitude(&self.x)
    }

    pub fn speed(&self) -> f64 {
        quat::norm(entry::velocity(&self.x))
    }

    /// Stagnation-point heat flux now, W/m².
    pub fn heat_flux(&self) -> f64 {
        entry::heat_flux(entry::density(self.altitude()), self.speed())
    }

    /// Tile temperature now, K.
    pub fn tile_temperature(&self) -> f64 {
        entry::tile_temperature(self.heat_flux())
    }

    /// Advance `dt` seconds (in steps of at most ENTRY_STEP).
    pub fn step(&mut self, dt: f64) {
        let mut left = dt;
        while left > 1e-9 && self.outcome.is_none() {
            let h = left.min(ENTRY_STEP);
            left -= h;
            let turn = (self.bank_cmd - self.bank).clamp(-BANK_RATE * h, BANK_RATE * h);
            self.bank += turn;
            self.x = entry::rk4(&self.aero, &self.x, self.mass, self.bank, h);
            self.t += h;
            let q = self.heat_flux();
            let tile = entry::tile_temperature(q);
            self.heat_load += q * h;
            self.peak_heat_flux = self.peak_heat_flux.max(q);
            self.peak_tile = self.peak_tile.max(tile);
            let l = entry::loads(&self.aero, &self.x, self.mass);
            self.peak_g = self.peak_g.max(l.lift.hypot(l.drag) / model::vehicle::G0);
            if tile > entry::TILE_LIMIT {
                self.overheat += h;
                if self.overheat > entry::OVERHEAT_TIME {
                    self.outcome = Some(EntryOutcome::BurnedThrough);
                }
            }
            if self.altitude() <= self.handover_altitude {
                self.outcome = Some(EntryOutcome::HandOver);
            }
        }
    }
}
