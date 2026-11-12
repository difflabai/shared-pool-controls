//! Synthetic, enforced adapter. It holds no real assets and runs no real jobs.
use near_sdk::{env, near, AccountId, Gas, NearToken, PanicOnDefault};
/// The lock retains its 256 most recent records (pool_core::MAX_RECORDS). A
/// job whose id is this far behind the newest id `execute` has accepted
/// belongs to a pruned, terminal record, so it is forgotten.
const WINDOW: u64 = 256;
/// Jobs held at most: up to WINDOW within the window, plus ids above the
/// newest executed id that the operator confirmed as not executed.
const MAX_JOBS: usize = 512;
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
    /// The largest id `execute` has accepted from the lock.
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
    /// Starts job `id` once; a repeated call returns the stored status. Jobs
    /// 256 or more ids behind the newest executed id are forgotten, and such
    /// an id returns "cancelled" without starting anything. The lock never
    /// dispatches it: its record was pruned, so it is terminal.
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
        // Only the lock moves `newest`, never operator input.
        self.newest = self.newest.max(id);
        let newest = u64::from(self.newest);
        self.jobs.retain(|j| u64::from(j.id) + WINDOW > newest);
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
    /// Independent provider/acceptor attests exact completion or definitive cancellation.
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
    /// Definitive evidence for an execute receipt that failed before creating a job.
    /// A forgotten id fails "stale id". The job cap (512) is reached only after
    /// at least 256 confirmations of ids above the newest executed id, with no
    /// `execute` in between; one successful `execute` of an id at least as
    /// high as the confirmed ones frees the space.
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
}
impl Executor {
    fn operator(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.operator,
            "only independent operator"
        );
    }
    /// Calls the lock's `settle({id,completed})` with 40 Tgas and makes its
    /// result the result of the current call. NT-F3: near-sdk's low-level
    /// promise functions, not its `Promise` type, which links the host
    /// functions for every action (deploy, keys, transfer, stake, accounts).
    fn notify(&self, id: u32, completed: bool) {
        let settle = env::promise_create(
            self.lock.clone(),
            "settle",
            near_sdk::serde_json::to_vec(
                &near_sdk::serde_json::json!({"id":id,"completed":completed}),
            )
            .unwrap(),
            NearToken::from_yoctonear(0),
            Gas::from_tgas(40),
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
    use near_sdk::testing_env;
    fn caller(account: &str) {
        let mut b = VMContextBuilder::new();
        b.current_account_id("executor.test.near".parse().unwrap())
            .predecessor_account_id(account.parse().unwrap());
        testing_env!(b.build());
    }
    /// Every receipt created by the last call, as one function call each:
    /// (receiver, method, JSON args, deposit in yocto, gas in Tgas, unused-gas weight).
    fn calls() -> Vec<(String, String, Value, u128, u64, u64)> {
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
                }] if r.receipt_indices.is_empty() => (
                    r.receiver_id.to_string(),
                    String::from_utf8(method_name.clone()).unwrap(),
                    near_sdk::serde_json::from_slice(args).unwrap(),
                    attached_deposit.as_yoctonear(),
                    prepaid_gas.as_tgas(),
                    gas_weight.0,
                ),
                _ => panic!("not one independent function call: {r:?}"),
            })
            .collect()
    }
    fn settle(id: u32, completed: bool) -> (String, String, Value, u128, u64, u64) {
        let args = json!({"id":id,"completed":completed});
        ("pool.test.near".into(), "settle".into(), args, 0, 40, 0)
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
        assert_eq!(calls(), vec![]);
        caller("operator.test.near");
        e.finish(0, true);
        assert_eq!(calls(), vec![settle(0, true)]);
        caller("operator.test.near");
        e.confirm_not_executed(1, 0, 2);
        assert_eq!(calls(), vec![settle(1, false)]);
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
        assert_eq!(calls(), vec![]);
    }
    /// After an execute of id 256, id 1 can still be confirmed as not
    /// executed (id 0 is stale, below). Operator input never moves the
    /// newest id: a confirmation of a far higher id, such as a typo, makes
    /// nothing stale.
    #[test]
    fn f3_confirm_is_accepted_up_to_255_ids_behind() {
        let mut e = executor();
        caller("pool.test.near");
        e.execute(256, 0, 1);
        caller("operator.test.near");
        e.confirm_not_executed(1_000_000, 0, 1);
        assert_eq!(e.newest, 256);
        caller("operator.test.near");
        e.confirm_not_executed(1, 0, 1);
        assert_eq!(ids(&e), vec![256, 1_000_000, 1]);
        assert_eq!(calls(), vec![settle(1, false)]);
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
    /// The cap: 512 jobs confirmed above the newest executed id fill it.
    #[test]
    #[should_panic(expected = "full")]
    fn f3_confirmations_above_the_newest_fill_the_cap() {
        let mut e = executor();
        for id in 1..=513 {
            caller("operator.test.near");
            e.confirm_not_executed(id, 0, 1);
        }
    }
    /// One execute of a higher id frees the cap.
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
}
