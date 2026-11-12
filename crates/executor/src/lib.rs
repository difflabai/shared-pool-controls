//! Synthetic, enforced adapter. It holds no real assets and runs no real jobs.
use near_sdk::{env, near, AccountId, Gas, NearToken, PanicOnDefault};
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
        assert!(self.jobs.len() < 256, "full");
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
    pub fn confirm_not_executed(&mut self, id: u32, recipe: u32, units: u32) {
        self.operator();
        assert!(!self.jobs.iter().any(|j| j.id == id), "job exists");
        assert!(self.jobs.len() < 256, "full");
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
}
