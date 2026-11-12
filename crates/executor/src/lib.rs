//! Synthetic, enforced adapter. It holds no real assets and runs no real jobs.
use near_sdk::{env, near, AccountId, Gas, NearToken, PanicOnDefault, Promise};
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
    pub fn finish(&mut self, id: u32, completed: bool) -> Promise {
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
        self.notify(id, completed)
    }
    /// Definitive evidence for an execute receipt that failed before creating a job.
    pub fn confirm_not_executed(&mut self, id: u32, recipe: u32, units: u32) -> Promise {
        self.operator();
        assert!(!self.jobs.iter().any(|j| j.id == id), "job exists");
        assert!(self.jobs.len() < 256, "full");
        self.jobs.push(Job {
            id,
            recipe,
            units,
            status: "cancelled".into(),
        });
        self.notify(id, false)
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
    fn notify(&self, id: u32, completed: bool) -> Promise {
        Promise::new(self.lock.clone()).function_call(
            "settle",
            near_sdk::serde_json::to_vec(
                &near_sdk::serde_json::json!({"id":id,"completed":completed}),
            )
            .unwrap(),
            NearToken::from_yoctonear(0),
            Gas::from_tgas(40),
        )
    }
}
