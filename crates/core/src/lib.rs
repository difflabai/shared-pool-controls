//! Bounded, dependency-free decision kernel. Serialization is optional.
//! See docs/spec.md for the transition system and explicit trust assumptions.
#![forbid(unsafe_code)]

#[cfg(test)]
mod findings;
pub mod fixtures;
#[cfg(test)]
mod tests;

pub const MAX_METRICS: usize = 32;
pub const MAX_RECIPES: usize = 32;
pub const MAX_TERMS: usize = 64;
pub const MAX_RECORDS: usize = 256;
pub const MAX_VALUE: i64 = 1_000_000;
pub const MAX_WEIGHT: u32 = 1_000;
pub const MAX_UNITS: u32 = 100;
/// Submit, dispatch, set_paused and observe fail Arithmetic from this
/// version on. The 3 * MAX_RECORDS versions above it are a reserve: an open
/// record needs at most three more increments to become terminal
/// (Dispatched -> Unknown -> Active -> terminal), so acknowledgment,
/// settlement and cancellation can never overflow the version.
pub const VERSION_CAP: u32 = u32::MAX - 3 * MAX_RECORDS as u32;

macro_rules! data {
    ($($item:item)*) => {$ (
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[cfg_attr(feature="serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature="borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
        $item
    )*};
}
data! {
    pub struct Metric { pub name: String, pub unit: String, pub initial: i64, pub min: i64, pub max: i64 }
    pub enum Shape { Target, Excess, Deficit }
    pub struct Term { pub metric: u32, pub target: i64, pub weight: u32, pub shape: Shape }
    /// Owner-defined effects, never supplied by an agent. Each vector spans all metrics.
    pub struct Recipe {
        pub name: String, pub actor: String, pub executor: String,
        pub reserve: Vec<i64>, pub complete: Vec<i64>, pub max_units: u32,
        pub requires: Vec<(u32,u32)>,
    }
    pub struct Config { pub name: String, pub owner: String, pub metrics: Vec<Metric>, pub terms: Vec<Term>, pub recipes: Vec<Recipe>, pub max_age_seconds: u32 }
    pub struct Request { pub recipe: u32, pub units: u32, pub version: u32, pub expires_at: u64 }
    pub enum Status { Reserved, Dispatched, Active, Unknown, Completed, Cancelled }
    pub struct Commitment { pub id: u32, pub recipe: u32, pub units: u32, pub expires_at: u64, pub status: Status }
    pub struct Decision { pub allowed: bool, pub reason: String, pub before: String, pub after: Option<String>, pub terms_before: Vec<String>, pub terms_after: Vec<String>, pub candidate: Option<Vec<i64>> }
    pub struct Pool { pub config: Config, pub values: Vec<i64>, pub version: u32, pub observed_at: u64, pub paused: bool, pub records: Vec<Commitment> }
    pub enum Error { InvalidConfig, Unauthorized, Paused, Stale, Expired, SnapshotExpired, InvalidRecipe, InvalidUnits, Prerequisite, Full, Arithmetic, Bounds, Energy, InvalidState, NotFound, Pending }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

fn bounded(v: i64) -> bool {
    (-MAX_VALUE..=MAX_VALUE).contains(&v)
}
fn short(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128
}
fn open(s: &Status) -> bool {
    !matches!(s, Status::Completed | Status::Cancelled)
}
fn add(base: &[i64], delta: &[i64], units: u32) -> Result<Vec<i64>, Error> {
    if base.len() != delta.len() {
        return Err(Error::InvalidConfig);
    }
    base.iter()
        .zip(delta)
        .map(|(x, d)| {
            let v = x
                .checked_add(d.checked_mul(i64::from(units)).ok_or(Error::Arithmetic)?)
                .ok_or(Error::Arithmetic)?;
            if bounded(v) {
                Ok(v)
            } else {
                Err(Error::Arithmetic)
            }
        })
        .collect()
}
impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        let n = self.metrics.len();
        if !short(&self.name)
            || !short(&self.owner)
            || n == 0
            || n > MAX_METRICS
            || self.recipes.is_empty()
            || self.recipes.len() > MAX_RECIPES
            || self.terms.is_empty()
            || self.terms.len() > MAX_TERMS
        {
            return Err(Error::InvalidConfig);
        }
        for m in &self.metrics {
            if !short(&m.name)
                || !short(&m.unit)
                || !bounded(m.min)
                || !bounded(m.max)
                || m.min > m.max
                || m.initial < m.min
                || m.initial > m.max
            {
                return Err(Error::InvalidConfig);
            }
        }
        for t in &self.terms {
            if t.metric as usize >= n
                || !bounded(t.target)
                || t.weight == 0
                || t.weight > MAX_WEIGHT
            {
                return Err(Error::InvalidConfig);
            }
        }
        for (i, r) in self.recipes.iter().enumerate() {
            // An actor that is also its recipe's executor would attest its own work.
            if !short(&r.name)
                || !short(&r.actor)
                || !short(&r.executor)
                || r.actor == r.executor
                || r.reserve.len() != n
                || r.complete.len() != n
                || r.max_units == 0
                || r.max_units > MAX_UNITS
                || r.requires.len() > MAX_RECIPES
                || r.requires
                    .iter()
                    .any(|(p, u)| *p as usize >= i || *u == 0 || *u > MAX_UNITS)
                || r.reserve.iter().chain(&r.complete).any(|v| !bounded(*v))
            {
                return Err(Error::InvalidConfig);
            }
            // Strictly earlier prerequisites give a DAG; duplicate IDs are not meaningful.
            for (j, p) in r.requires.iter().enumerate() {
                if r.requires[..j].iter().any(|v| v.0 == p.0) {
                    return Err(Error::InvalidConfig);
                }
            }
        }
        Ok(())
    }
}
impl Pool {
    pub fn new(config: Config, now: u64) -> Result<Self, Error> {
        config.validate()?;
        let values = config.metrics.iter().map(|m| m.initial).collect();
        Ok(Self {
            config,
            values,
            version: 0,
            observed_at: now,
            paused: false,
            records: vec![],
        })
    }
    pub fn energy_terms(&self, values: &[i64]) -> Result<Vec<u64>, Error> {
        if values.len() != self.config.metrics.len() || values.iter().any(|v| !bounded(*v)) {
            return Err(Error::Bounds);
        }
        self.config
            .terms
            .iter()
            .map(|t| {
                let d = values[t.metric as usize] - t.target;
                let r = match t.shape {
                    Shape::Target => d.unsigned_abs(),
                    Shape::Excess => d.max(0) as u64,
                    Shape::Deficit => (-d).max(0) as u64,
                };
                r.checked_mul(r)
                    .and_then(|v| v.checked_mul(u64::from(t.weight)))
                    .ok_or(Error::Arithmetic)
            })
            .collect()
    }
    pub fn energy(&self, values: &[i64]) -> Result<u64, Error> {
        self.energy_terms(values)?
            .iter()
            .try_fold(0u64, |a, b| a.checked_add(*b).ok_or(Error::Arithmetic))
    }
    fn violations(&self, values: &[i64]) -> Vec<u64> {
        values
            .iter()
            .zip(&self.config.metrics)
            .map(|(v, m)| (m.min - v).max(0) as u64 + (v - m.max).max(0) as u64)
            .collect()
    }
    /// Per metric, an interval that contains every outcome of every open record
    /// (it stays open, completes, or releases its reservation) and of `extra`,
    /// with record `skip` left out. A pending hedge gives no hard-limit headroom
    /// before it executes, and no admission can rely on another record's
    /// reservation staying in place.
    fn envelope(
        &self,
        values: &[i64],
        extra: Option<(&Recipe, u32)>,
        skip: Option<u32>,
    ) -> Result<(Vec<i64>, Vec<i64>), Error> {
        let mut low = values.to_vec();
        let mut high = values.to_vec();
        let mut apply = |r: &Recipe, units: u32| -> Result<(), Error> {
            for i in 0..values.len() {
                // d: completion delta; x: release delta (cancel or settle(false)).
                let d = r.complete[i]
                    .checked_mul(i64::from(units))
                    .ok_or(Error::Arithmetic)?;
                let x = r.reserve[i]
                    .checked_mul(-i64::from(units))
                    .ok_or(Error::Arithmetic)?;
                low[i] = low[i]
                    .checked_add(d.min(x).min(0))
                    .ok_or(Error::Arithmetic)?;
                high[i] = high[i]
                    .checked_add(d.max(x).max(0))
                    .ok_or(Error::Arithmetic)?;
                if !bounded(low[i]) || !bounded(high[i]) {
                    return Err(Error::Arithmetic);
                }
            }
            Ok(())
        };
        for c in &self.records {
            if open(&c.status) && Some(c.id) != skip {
                apply(&self.config.recipes[c.recipe as usize], c.units)?;
            }
        }
        if let Some((r, u)) = extra {
            apply(r, u)?;
        }
        Ok((low, high))
    }
    /// No metric's interval violation may exceed its violation now or at
    /// `candidate`. Since every admission passes this check, in every reachable
    /// state each metric's interval violation equals its current violation, and
    /// every completion or release of an open record passes this check too
    /// (LIVE, docs/verification-handoff.md).
    fn check_bounds(
        &self,
        candidate: &[i64],
        extra: Option<(&Recipe, u32)>,
        skip: Option<u32>,
    ) -> Result<(), Error> {
        let (low, high) = self.envelope(candidate, extra, skip)?;
        let old = self.violations(&self.values);
        let new = self.violations(candidate);
        for (i, m) in self.config.metrics.iter().enumerate() {
            let v = (m.min - low[i]).max(0) as u64 + (high[i] - m.max).max(0) as u64;
            if v > old[i].min(new[i]) {
                return Err(Error::Bounds);
            }
        }
        Ok(())
    }
    fn candidate(&self, caller: &str, req: &Request, now: u64) -> Result<Vec<i64>, Error> {
        if self.paused {
            return Err(Error::Paused);
        }
        if req.version != self.version {
            return Err(Error::Stale);
        }
        if req.expires_at <= now || req.expires_at > now.saturating_add(86_400) {
            return Err(Error::Expired);
        }
        if self.config.max_age_seconds > 0
            && now
                > self
                    .observed_at
                    .saturating_add(u64::from(self.config.max_age_seconds))
        {
            return Err(Error::SnapshotExpired);
        }
        if self.records.len() >= MAX_RECORDS {
            return Err(Error::Full);
        }
        let r = self
            .config
            .recipes
            .get(req.recipe as usize)
            .ok_or(Error::InvalidRecipe)?;
        if caller != r.actor {
            return Err(Error::Unauthorized);
        }
        if req.units == 0 || req.units > r.max_units {
            return Err(Error::InvalidUnits);
        }
        if r.requires.iter().any(|(p, units)| {
            self.records
                .iter()
                .filter(|c| c.recipe == *p && c.status == Status::Completed)
                .map(|c| c.units)
                .sum::<u32>()
                < *units
        }) {
            return Err(Error::Prerequisite);
        }
        let next = add(&self.values, &r.reserve, req.units)?;
        self.check_bounds(&next, Some((r, req.units)), None)?;
        let before = self.energy(&self.values)?;
        let after = self.energy(&next)?;
        if after > before {
            return Err(Error::Energy);
        }
        if self.violations(&self.values).iter().any(|v| *v > 0)
            && self.energy(&add(&next, &r.complete, req.units)?)? >= before
        {
            return Err(Error::Energy);
        }
        if self.version >= VERSION_CAP {
            return Err(Error::Arithmetic);
        }
        Ok(next)
    }
    pub fn preview(&self, caller: &str, req: &Request, now: u64) -> Decision {
        let raw = self.config.recipes.get(req.recipe as usize).and_then(|r| {
            if req.units <= MAX_UNITS {
                add(&self.values, &r.reserve, req.units).ok()
            } else {
                None
            }
        });
        let result = self.candidate(caller, req, now);
        Decision {
            allowed: result.is_ok(),
            reason: result
                .err()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Allowed".into()),
            before: self.energy(&self.values).unwrap_or(0).to_string(),
            after: raw
                .as_ref()
                .and_then(|v| self.energy(v).ok())
                .map(|v| v.to_string()),
            terms_before: self
                .energy_terms(&self.values)
                .unwrap_or_default()
                .iter()
                .map(u64::to_string)
                .collect(),
            terms_after: raw
                .as_ref()
                .and_then(|v| self.energy_terms(v).ok())
                .unwrap_or_default()
                .iter()
                .map(u64::to_string)
                .collect(),
            candidate: raw,
        }
    }
    pub fn submit(&mut self, caller: &str, req: Request, now: u64) -> Result<u32, Error> {
        let next = self.candidate(caller, &req, now)?;
        let version = self.version.checked_add(1).ok_or(Error::Arithmetic)?;
        let id = self.records.len() as u32;
        self.records.push(Commitment {
            id,
            recipe: req.recipe,
            units: req.units,
            expires_at: req.expires_at,
            status: Status::Reserved,
        });
        self.values = next;
        self.version = version;
        Ok(id)
    }
    fn record(&self, id: u32) -> Result<&Commitment, Error> {
        self.records.get(id as usize).ok_or(Error::NotFound)
    }
    fn bump(&mut self) -> Result<(), Error> {
        self.version = self.version.checked_add(1).ok_or(Error::Arithmetic)?;
        Ok(())
    }
    /// The increment for calls that start or administer work. It stops at
    /// VERSION_CAP, which keeps the reserve for resolving open records.
    fn bump_new(&mut self) -> Result<(), Error> {
        if self.version >= VERSION_CAP {
            return Err(Error::Arithmetic);
        }
        self.bump()
    }
    pub fn dispatch(&mut self, caller: &str, id: u32, now: u64) -> Result<(), Error> {
        let c = self.record(id)?;
        let r = &self.config.recipes[c.recipe as usize];
        if caller != r.actor {
            return Err(Error::Unauthorized);
        }
        if self.paused {
            return Err(Error::Paused);
        }
        if c.status != Status::Reserved {
            return Err(Error::InvalidState);
        }
        if now >= c.expires_at {
            return Err(Error::Expired);
        }
        if self.config.max_age_seconds > 0
            && now
                > self
                    .observed_at
                    .saturating_add(u64::from(self.config.max_age_seconds))
        {
            return Err(Error::SnapshotExpired);
        }
        self.bump_new()?;
        self.records[id as usize].status = Status::Dispatched;
        Ok(())
    }
    /// Wrapper must only call this from its private runtime callback.
    pub fn acknowledge(&mut self, id: u32, accepted: bool) -> Result<(), Error> {
        let c = self.record(id)?;
        if !matches!(c.status, Status::Dispatched | Status::Unknown) {
            return Ok(());
        }
        let next = if accepted {
            Status::Active
        } else {
            Status::Unknown
        };
        // reconcile is permissionless: a no-op must not bump the version, or
        // anyone could repeatedly invalidate every in-flight request.
        if self.records[id as usize].status == next {
            return Ok(());
        }
        self.bump()?;
        self.records[id as usize].status = next;
        Ok(())
    }
    pub fn cancel(&mut self, caller: &str, id: u32, now: u64) -> Result<(), Error> {
        let c = self.record(id)?;
        let r = &self.config.recipes[c.recipe as usize];
        if caller != r.actor && caller != self.config.owner {
            return Err(Error::Unauthorized);
        }
        if c.status == Status::Cancelled {
            return Ok(());
        }
        if c.status != Status::Reserved {
            return Err(Error::Pending);
        }
        let _ = now;
        self.release(id)
    }
    fn release(&mut self, id: u32) -> Result<(), Error> {
        let c = self.record(id)?;
        let r = &self.config.recipes[c.recipe as usize];
        let undo: Vec<i64> = r.reserve.iter().map(|d| -d).collect();
        let next = add(&self.values, &undo, c.units)?;
        // The release is still checked. Admission counted it in the envelope,
        // so in a reachable state this check passes; no release is forced.
        self.check_bounds(&next, None, Some(id))?;
        self.bump()?;
        self.values = next;
        self.records[id as usize].status = Status::Cancelled;
        Ok(())
    }
    /// Authenticated executor attests completion or definitive nonexecution.
    /// Unknown outcomes must not call this with completed=false.
    pub fn settle(&mut self, caller: &str, id: u32, completed: bool) -> Result<(), Error> {
        let c = self.record(id)?;
        let r = &self.config.recipes[c.recipe as usize];
        if caller != r.executor {
            return Err(Error::Unauthorized);
        }
        if (completed && c.status == Status::Completed)
            || (!completed && c.status == Status::Cancelled)
        {
            return Ok(());
        }
        if !matches!(
            c.status,
            Status::Dispatched | Status::Active | Status::Unknown
        ) {
            return Err(Error::InvalidState);
        }
        if !completed {
            return self.release(id);
        }
        let next = add(&self.values, &r.complete, c.units)?;
        self.check_bounds(&next, None, Some(id))?;
        self.bump()?;
        self.values = next;
        self.records[id as usize].status = Status::Completed;
        Ok(())
    }
    pub fn set_paused(&mut self, caller: &str, paused: bool) -> Result<(), Error> {
        if caller != self.config.owner {
            return Err(Error::Unauthorized);
        }
        self.bump_new()?;
        self.paused = paused;
        Ok(())
    }
    /// New externally observed baseline, only when no obligations are unresolved.
    /// Restating the current values is allowed at any time: it changes no
    /// value or record and renews the snapshot timestamp. Does not change
    /// immutable policy. New values may expose a hard-limit violation.
    pub fn observe(&mut self, caller: &str, values: Vec<i64>, now: u64) -> Result<(), Error> {
        if caller != self.config.owner {
            return Err(Error::Unauthorized);
        }
        if values != self.values && self.records.iter().any(|c| open(&c.status)) {
            return Err(Error::Pending);
        }
        if values.len() != self.values.len() || values.iter().any(|v| !bounded(*v)) {
            return Err(Error::Bounds);
        }
        self.energy(&values)?;
        self.bump_new()?;
        self.values = values;
        self.observed_at = now;
        Ok(())
    }
    /// Offchain bounded search. This is not part of the admission authority.
    pub fn best_quantity(&self, caller: &str, request: &Request, now: u64) -> Option<Request> {
        (1..=request.units.min(MAX_UNITS))
            .filter_map(|units| {
                let req = Request {
                    units,
                    ..request.clone()
                };
                self.candidate(caller, &req, now)
                    .ok()
                    .and_then(|v| self.energy(&v).ok())
                    .map(|e| (e, std::cmp::Reverse(units), req))
            })
            .min_by_key(|(e, u, _)| (*e, *u))
            .map(|(_, _, r)| r)
    }
}
