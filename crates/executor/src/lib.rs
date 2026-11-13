//! Synthetic, enforced adapter. It holds no real assets and runs no real jobs.
use near_sdk::{env, near, AccountId, Gas, NearToken, PanicOnDefault, PromiseError};
/// The lock retains its 256 most recent records (pool_core::MAX_RECORDS). A
/// job whose id is this far behind `newest` belongs to a pruned, terminal
/// record, so it is forgotten.
const WINDOW: u64 = 256;
/// Jobs held at most. Job ids are distinct and each is above
/// `newest - WINDOW`, so at most WINDOW of them are at or below `newest`.
/// Every other job is a confirmation above `newest` whose `on_settled` has
/// not run yet, or whose `settle` the lock refused. So the cap needs at
/// least WINDOW such jobs above `newest`.
const MAX_JOBS: usize = 512;
/// Gas for the lock's `settle`, and for this contract's `on_settled` after it.
const SETTLE_TGAS: u64 = 40;
const ON_SETTLED_TGAS: u64 = 10;
#[near(serializers=[borsh,json])]
#[derive(Clone)]
pub struct Job {
    id: u32,
    recipe: u32,
    units: u32,
    status: String,
}
#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Executor {
    lock: AccountId,
    operator: AccountId,
    jobs: Vec<Job>,
    mode: String,
    /// The largest id the lock has vouched for: one it sent to `execute`, or
    /// one whose `settle` from this executor it accepted. Either way the lock
    /// retained that record then, so every id WINDOW or more below it was
    /// already pruned.
    newest: u32,
}
#[near]
impl Executor {
    #[init]
    #[private]
    pub fn new(lock: AccountId, operator: AccountId) -> Self {
        Self {
            lock,
            operator,
            jobs: vec![],
            mode: "normal".into(),
            newest: 0,
        }
    }
    pub fn set_mode(&mut self, mode: String) {
        self.operator();
        assert!(
            ["normal", "reject", "unknown", "panic"].contains(&mode.as_str()),
            "invalid mode"
        );
        self.mode = mode;
    }
    /// Starts job `id` once; a repeated call returns the stored status. A job
    /// is forgotten once `newest` is WINDOW or more past its id. Such an id
    /// returns "cancelled" without starting anything: the lock has pruned its
    /// record, so it is terminal and never dispatched again.
    pub fn execute(&mut self, id: u32, recipe: u32, units: u32) -> String {
        assert_eq!(
            env::predecessor_account_id(),
            self.lock,
            "only lock may execute"
        );
        if let Some(j) = self.jobs.iter().find(|j| j.id == id) {
            assert!(j.recipe == recipe && j.units == units, "payload mismatch");
            return j.status.clone();
        }
        // A forgotten id belongs to a pruned record: never start it.
        if u64::from(id) + WINDOW <= u64::from(self.newest) {
            return "cancelled".into();
        }
        // The lock dispatched `id`, so it retains the record.
        self.advance(id);
        assert!(self.jobs.len() < MAX_JOBS, "full");
        assert_ne!(self.mode, "panic", "injected failure");
        let status = if self.mode == "reject" {
            "cancelled"
        } else {
            "active"
        }
        .to_string();
        self.jobs.push(Job {
            id,
            recipe,
            units,
            status: status.clone(),
        });
        if self.mode == "unknown" {
            "unknown".into()
        } else {
            status
        }
    }
    pub fn status(&self, id: u32) -> String {
        self.jobs
            .iter()
            .find(|j| j.id == id)
            .map(|j| j.status.clone())
            .unwrap_or_else(|| "unknown".into())
    }
    /// Independent provider/acceptor attests exact completion or definitive
    /// cancellation. If the lock accepts the settle, `on_settled` raises
    /// `newest` to `id`; this is also how a refused confirmation is retried.
    pub fn finish(&mut self, id: u32, completed: bool) {
        self.operator();
        let j = self
            .jobs
            .iter_mut()
            .find(|j| j.id == id)
            .expect("unknown job");
        let next = if completed { "completed" } else { "cancelled" };
        assert!(
            j.status == "active" || j.status == next,
            "terminal outcome conflict"
        );
        j.status = next.into();
        self.notify(id, completed);
    }
    /// Definitive evidence for an execute receipt that failed before creating
    /// a job. A forgotten id fails "stale id". The job is stored first and kept
    /// whatever the lock answers. If the lock accepts the settle, `on_settled`
    /// raises `newest` to `id`. If it refuses (an unassigned or pruned id, a
    /// record still Reserved, another outcome already recorded), the job stays
    /// until `newest` is WINDOW or more past its id, for good if the lock never
    /// reaches that id. MAX_JOBS is reached only with at least WINDOW jobs
    /// above `newest`, refused or still waiting for `on_settled`.
    pub fn confirm_not_executed(&mut self, id: u32, recipe: u32, units: u32) {
        self.operator();
        assert!(!self.jobs.iter().any(|j| j.id == id), "job exists");
        assert!(u64::from(id) + WINDOW > u64::from(self.newest), "stale id");
        assert!(self.jobs.len() < MAX_JOBS, "full");
        self.jobs.push(Job {
            id,
            recipe,
            units,
            status: "cancelled".into(),
        });
        self.notify(id, false);
    }
    pub fn get_jobs(&self) -> Vec<Job> {
        self.jobs.clone()
    }
    /// Private callback of `notify`, after the lock's `settle({id,..})`. The
    /// lock accepts a settle only for a record it retains, so on success every
    /// id WINDOW or more below `id` is pruned there: raise `newest`, as
    /// `execute` does. A refused settle changes nothing; the transaction
    /// already reports the lock's error. `#[private]` admits only this
    /// account, and only `notify` gives it a promise result.
    #[private]
    pub fn on_settled(&mut self, id: u32) {
        assert_eq!(env::promise_results_count(), 1, "not a settle callback");
        // The lock's settle returns no value; any success counts.
        if env::promise_result_checked(0, 0) != Err(PromiseError::Failed) {
            self.advance(id);
        }
    }
}
impl Executor {
    fn operator(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.operator,
            "only independent operator"
        );
    }
    /// The lock retained record `id` at some moment, so every id WINDOW or
    /// more below it was pruned then: terminal, and never dispatched again.
    /// Raises `newest` to `id` and forgets those jobs.
    fn advance(&mut self, id: u32) {
        self.newest = self.newest.max(id);
        let newest = u64::from(self.newest);
        self.jobs.retain(|j| u64::from(j.id) + WINDOW > newest);
    }
    /// Calls the lock's `settle({id,completed})`, then this contract's
    /// `on_settled({id})`. The call's result stays the lock's (Ok, or its
    /// error): `on_settled` waits on `settle` but is not returned. NT-F3:
    /// low-level promise functions only, never near-sdk's `Promise` type.
    fn notify(&self, id: u32, completed: bool) {
        let zero = NearToken::from_yoctonear(0);
        let args = |v: near_sdk::serde_json::Value| near_sdk::serde_json::to_vec(&v).unwrap();
        let settle = env::promise_create(
            self.lock.clone(),
            "settle",
            args(near_sdk::serde_json::json!({"id":id,"completed":completed})),
            zero,
            Gas::from_tgas(SETTLE_TGAS),
        );
        env::promise_then(
            settle,
            env::current_account_id(),
            "on_settled",
            args(near_sdk::serde_json::json!({"id":id})),
            zero,
            Gas::from_tgas(ON_SETTLED_TGAS),
        );
        env::promise_return(settle);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::mock::MockAction;
    use near_sdk::serde_json::{json, Value};
    use near_sdk::test_utils::{get_created_receipts, VMContextBuilder};
    use near_sdk::{testing_env, PromiseResult};
    fn caller(account: &str) {
        let mut b = VMContextBuilder::new();
        b.current_account_id("executor.test.near".parse().unwrap())
            .predecessor_account_id(account.parse().unwrap());
        testing_env!(b.build());
    }
    /// A receipt holding one function call: (receiver, receipts it waits for,
    /// method, JSON args, deposit in yocto, gas in Tgas, unused-gas weight).
    type Receipt = (String, Vec<u64>, String, Value, u128, u64, u64);
    /// Every receipt created by the last call.
    fn receipts() -> Vec<Receipt> {
        get_created_receipts()
            .into_iter()
            .map(|r| match &r.actions[..] {
                [MockAction::FunctionCallWeight {
                    method_name,
                    args,
                    attached_deposit,
                    prepaid_gas,
                    gas_weight,
                    ..
                }] => (
                    r.receiver_id.to_string(),
                    r.receipt_indices.clone(),
                    String::from_utf8(method_name.clone()).unwrap(),
                    near_sdk::serde_json::from_slice(args).unwrap(),
                    attached_deposit.as_yoctonear(),
                    prepaid_gas.as_tgas(),
                    gas_weight.0,
                ),
                _ => panic!("not one function call: {r:?}"),
            })
            .collect()
    }
    /// What `notify` creates: the lock's settle (40 Tgas), then this
    /// contract's `on_settled` (10 Tgas), which waits for it.
    fn settle(id: u32, completed: bool) -> Vec<Receipt> {
        vec![
            (
                "pool.test.near".into(),
                vec![],
                "settle".into(),
                json!({"id":id,"completed":completed}),
                0,
                40,
                0,
            ),
            (
                "executor.test.near".into(),
                vec![0],
                "on_settled".into(),
                json!({"id":id}),
                0,
                10,
                0,
            ),
        ]
    }
    /// The executor's own `on_settled(id)` after the lock accepted (`ok`) or
    /// refused its settle.
    fn settled(e: &mut Executor, id: u32, ok: bool) {
        let mut b = VMContextBuilder::new();
        b.current_account_id("executor.test.near".parse().unwrap())
            .predecessor_account_id("executor.test.near".parse().unwrap());
        let result = if ok {
            PromiseResult::Successful(vec![])
        } else {
            PromiseResult::Failed
        };
        testing_env!(
            b.build(),
            near_sdk::test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            Default::default(),
            vec![result]
        );
        e.on_settled(id);
    }
    fn confirm(e: &mut Executor, id: u32) {
        caller("operator.test.near");
        e.confirm_not_executed(id, 0, 1);
    }
    /// The lock executes each id in `ids`; each starts.
    fn execute(e: &mut Executor, ids: impl IntoIterator<Item = u32>) {
        caller("pool.test.near");
        for id in ids {
            assert_eq!(e.execute(id, 0, 1), "active");
        }
    }
    /// The executor's stored state, as bytes.
    fn stored(e: &Executor) -> Vec<u8> {
        near_sdk::borsh::to_vec(e).unwrap()
    }
    #[test]
    fn finish_and_confirm_call_settle_on_the_lock() {
        caller("executor.test.near");
        let mut e = Executor::new(
            "pool.test.near".parse().unwrap(),
            "operator.test.near".parse().unwrap(),
        );
        caller("pool.test.near");
        assert_eq!(e.execute(0, 0, 4), "active");
        assert_eq!(receipts(), vec![]);
        caller("operator.test.near");
        e.finish(0, true);
        assert_eq!(receipts(), settle(0, true));
        caller("operator.test.near");
        e.confirm_not_executed(1, 0, 2);
        assert_eq!(receipts(), settle(1, false));
        let jobs: Vec<_> = e.jobs.iter().map(|j| (j.id, j.status.as_str())).collect();
        assert_eq!(jobs, vec![(0, "completed"), (1, "cancelled")]);
    }
    fn executor() -> Executor {
        caller("executor.test.near");
        Executor::new(
            "pool.test.near".parse().unwrap(),
            "operator.test.near".parse().unwrap(),
        )
    }
    fn ids(e: &Executor) -> Vec<u32> {
        e.jobs.iter().map(|j| j.id).collect()
    }
    /// F-3: executing id N + 256 forgets job N. A later execute of N starts
    /// nothing and returns "cancelled"; retained jobs keep their status.
    #[test]
    fn f3_execute_forgets_jobs_256_ids_behind_the_newest() {
        let mut e = executor();
        caller("pool.test.near");
        for id in 0..256 {
            assert_eq!(e.execute(id, 0, 1), "active");
        }
        assert_eq!(ids(&e), (0..256).collect::<Vec<_>>());
        assert_eq!(e.execute(256, 0, 1), "active");
        assert_eq!((e.newest, ids(&e)), (256, (1..=256).collect::<Vec<_>>()));
        assert_eq!(e.status(0), "unknown");
        assert_eq!(e.execute(0, 0, 1), "cancelled");
        assert_eq!((e.newest, ids(&e)), (256, (1..=256).collect::<Vec<_>>()));
        assert_eq!(e.execute(1, 0, 1), "active");
        // A jump forgets every job up to the new id - 256.
        assert_eq!(e.execute(300, 0, 1), "active");
        let mut expected: Vec<u32> = (45..=256).collect();
        expected.push(300);
        assert_eq!(ids(&e), expected);
        // An id below the newest but within the window still starts once.
        assert_eq!(e.execute(299, 0, 1), "active");
        assert_eq!(e.newest, 300);
        assert_eq!(receipts(), vec![]);
    }
    /// After an execute of id 256, id 1 can still be confirmed as not
    /// executed (id 0 is stale, below). A confirmation the lock refuses, such
    /// as a typo of a far higher id, moves nothing, so nothing becomes stale.
    #[test]
    fn f3_confirm_is_accepted_up_to_255_ids_behind() {
        let mut e = executor();
        caller("pool.test.near");
        e.execute(256, 0, 1);
        caller("operator.test.near");
        e.confirm_not_executed(1_000_000, 0, 1);
        settled(&mut e, 1_000_000, false);
        assert_eq!(e.newest, 256);
        caller("operator.test.near");
        e.confirm_not_executed(1, 0, 1);
        assert_eq!(ids(&e), vec![256, 1_000_000, 1]);
        assert_eq!(receipts(), settle(1, false));
    }
    #[test]
    #[should_panic(expected = "stale id")]
    fn f3_stale_confirm_not_executed_panics() {
        let mut e = executor();
        caller("pool.test.near");
        e.execute(256, 0, 1);
        caller("operator.test.near");
        e.confirm_not_executed(0, 0, 1);
    }
    /// One execute of a higher id frees the cap: at most 256 jobs remain at
    /// or below the new newest id.
    #[test]
    fn f3_an_execute_frees_the_cap() {
        let mut e = executor();
        for id in 1..=512 {
            caller("operator.test.near");
            e.confirm_not_executed(id, 0, 1);
        }
        assert_eq!((e.jobs.len(), e.newest), (512, 0));
        caller("pool.test.near");
        assert_eq!(e.execute(600, 0, 1), "active");
        let mut expected: Vec<u32> = (345..=512).collect();
        expected.push(600);
        assert_eq!(ids(&e), expected);
        caller("operator.test.near");
        e.confirm_not_executed(601, 0, 1);
        assert_eq!(e.jobs.len(), 170);
    }
    /// D-1: the lock accepting a confirmation's settle moves `newest` and
    /// forgets every job 256 or more below it, as an execute does. An
    /// accepted settle of a lower id moves nothing.
    #[test]
    fn d1_an_accepted_settle_moves_newest_and_forgets() {
        let mut e = executor();
        execute(&mut e, 0..=255);
        confirm(&mut e, 300);
        assert_eq!((e.newest, e.jobs.len()), (255, 257));
        settled(&mut e, 300, true);
        let mut expected: Vec<u32> = (45..=255).collect();
        expected.push(300);
        assert_eq!((e.newest, ids(&e)), (300, expected.clone()));
        let before = stored(&e);
        settled(&mut e, 100, true);
        assert_eq!(stored(&e), before);
        // Id 44 is forgotten: its record was pruned, so nothing starts.
        caller("pool.test.near");
        assert_eq!(e.execute(44, 0, 1), "cancelled");
        assert_eq!((e.newest, ids(&e)), (300, expected));
    }
    /// D-1: a refused settle changes nothing, whichever job it was for.
    #[test]
    fn d1_a_refused_settle_changes_nothing() {
        let mut e = executor();
        execute(&mut e, 0..=10);
        confirm(&mut e, 400);
        assert_eq!(e.status(400), "cancelled");
        assert_eq!(e.newest, 10);
        // A confirmation job above the newest id, an executed job, an absent id.
        for id in [400, 5, 1_000] {
            let before = stored(&e);
            settled(&mut e, id, false);
            assert_eq!(stored(&e), before, "refused settle of {id}");
        }
        assert_eq!(e.newest, 10);
        assert_eq!(ids(&e).len(), 12);
    }
    /// D-1: `on_settled` needs the one promise result `notify` gives it. The
    /// executor account calling it directly has none. (`#[private]` exists
    /// only in the Wasm entry point; the sandbox tests it.)
    #[test]
    #[should_panic(expected = "not a settle callback")]
    fn d1_on_settled_needs_one_promise_result() {
        let mut e = executor();
        caller("executor.test.near");
        e.on_settled(5);
    }
    /// D-1, decision (a): `finish` also calls `on_settled`. A refused
    /// confirmation (as for a record still Reserved) moves nothing; the
    /// operator's retry `finish(id, false)`, once accepted, moves `newest`.
    #[test]
    fn d1_finish_moves_newest_too() {
        let mut e = executor();
        execute(&mut e, 0..=255);
        confirm(&mut e, 300);
        settled(&mut e, 300, false);
        assert_eq!((e.newest, e.jobs.len()), (255, 257));
        caller("operator.test.near");
        e.finish(300, false);
        assert_eq!(receipts(), settle(300, false));
        settled(&mut e, 300, true);
        let mut expected: Vec<u32> = (45..=255).collect();
        expected.push(300);
        assert_eq!((e.newest, ids(&e)), (300, expected));
    }
    /// D-1: a refused job is not freed when `newest` passes its id; it is
    /// forgotten only once `newest` is 256 or more past it.
    #[test]
    fn d1_a_refused_job_is_forgotten_256_past_its_id() {
        let mut e = executor();
        confirm(&mut e, 300);
        settled(&mut e, 300, false);
        execute(&mut e, [400]);
        assert_eq!(ids(&e), vec![300, 400]);
        confirm(&mut e, 555);
        settled(&mut e, 555, true);
        assert_eq!((e.newest, ids(&e)), (555, vec![300, 400, 555]));
        confirm(&mut e, 556);
        settled(&mut e, 556, true);
        assert_eq!((e.newest, ids(&e)), (556, vec![400, 555, 556]));
    }
    /// D-1: accepted confirmations alone keep at most 256 jobs (v1.1.0 before
    /// this change reached the 512-job cap after 512 of them).
    #[test]
    fn d1_accepted_confirmations_keep_256_jobs() {
        let mut e = executor();
        for id in 0..1_000u32 {
            confirm(&mut e, id);
            settled(&mut e, id, true);
            assert_eq!(e.jobs.len(), (id as usize + 1).min(256), "after {id}");
        }
        assert_eq!((e.newest, ids(&e)), (999, (744..=999).collect::<Vec<_>>()));
    }
    /// D-1: with 256 jobs at or below `newest`, 256 refused confirmations
    /// above it fill the cap, and the 257th fails "full".
    #[test]
    #[should_panic(expected = "full")]
    fn d1_256_refused_jobs_above_a_full_window_fill_the_cap() {
        let mut e = executor();
        execute(&mut e, 0..=255);
        for id in 300..556u32 {
            confirm(&mut e, id);
            settled(&mut e, id, false);
        }
        assert_eq!((e.newest, e.jobs.len()), (255, 512));
        confirm(&mut e, 556);
    }
    /// D-1: with fewer jobs at or below `newest`, more refused jobs fit above
    /// it: the cap counts all jobs.
    #[test]
    fn d1_with_fewer_jobs_below_newest_more_refused_jobs_fit() {
        let mut e = executor();
        execute(&mut e, 0..=99);
        for id in 300..712u32 {
            confirm(&mut e, id);
            settled(&mut e, id, false);
        }
        assert_eq!((e.newest, e.jobs.len()), (99, 512));
    }
    /// D-1: callbacks may run in any order; the result is the same.
    #[test]
    fn d1_callbacks_in_any_order() {
        let run = |reverse: bool| {
            let mut e = executor();
            for id in 0..300u32 {
                confirm(&mut e, id);
            }
            // All 300 confirmations in flight: no callback has run yet.
            assert_eq!((e.newest, e.jobs.len()), (0, 300));
            let mut order: Vec<u32> = (0..300).collect();
            if reverse {
                order.reverse();
            }
            for id in order {
                settled(&mut e, id, true);
            }
            assert_eq!((e.newest, ids(&e)), (299, (44..=299).collect::<Vec<_>>()));
            stored(&e)
        };
        assert_eq!(run(true), run(false));
    }
}
