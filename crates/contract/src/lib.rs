//! Thin NEAR shell: identity, storage, receipts, and asynchronous execution.
use near_sdk::{env, near, AccountId, Gas, PanicOnDefault, Promise, PromiseError};
use pool_core::{Commitment, Config, Decision, Pool, Request};

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    pool: Pool,
}

#[near(serializers=[json])]
pub struct Submission {
    pub decision: Decision,
    pub id: Option<u32>,
}

fn now() -> u64 {
    env::block_timestamp() / 1_000_000_000
}
fn caller() -> String {
    env::predecessor_account_id().to_string()
}
fn must<T>(result: Result<T, pool_core::Error>) -> T {
    result.unwrap_or_else(|e| env::panic_str(&e.to_string()))
}
fn event(kind: &str, data: near_sdk::serde_json::Value) {
    env::log_str(&format!(
        "EVENT_JSON:{}",
        near_sdk::serde_json::json!({"standard":"shared_pool","version":"1.0.0","event":kind,"data":[data]})
    ));
}
#[near]
impl Contract {
    #[init]
    #[private]
    pub fn new(config: Config) -> Self {
        config
            .owner
            .parse::<AccountId>()
            .expect("Invalid owner account");
        for r in &config.recipes {
            r.actor.parse::<AccountId>().expect("Invalid actor account");
            r.executor
                .parse::<AccountId>()
                .expect("Invalid executor account");
        }
        Self {
            pool: must(Pool::new(config, now())),
        }
    }
    pub fn get_pool(&self) -> Pool {
        self.pool.clone()
    }
    pub fn get_commitments(&self, from: u32, limit: u32) -> Vec<Commitment> {
        self.pool
            .records
            .iter()
            .skip(from as usize)
            .take(limit.min(50) as usize)
            .cloned()
            .collect()
    }
    pub fn preview(&self, actor: AccountId, request: Request) -> Decision {
        self.pool.preview(actor.as_str(), &request, now())
    }
    pub fn submit(&mut self, request: Request) -> Submission {
        let decision = self.pool.preview(&caller(), &request, now());
        let id = if decision.allowed {
            Some(must(self.pool.submit(&caller(), request.clone(), now())))
        } else {
            None
        };
        event(
            "decision",
            near_sdk::serde_json::json!({"actor":caller(),"request":request,"id":id,"allowed":decision.allowed,"reason":decision.reason,"before":decision.before,"after":decision.after,"version":self.pool.version}),
        );
        Submission { decision, id }
    }
    pub fn dispatch(&mut self, id: u32) -> Promise {
        must(self.pool.dispatch(&caller(), id, now()));
        let c = &self.pool.records[id as usize];
        let r = &self.pool.config.recipes[c.recipe as usize];
        let args = near_sdk::serde_json::to_vec(
            &near_sdk::serde_json::json!({"id":id,"recipe":c.recipe,"units":c.units}),
        )
        .unwrap();
        Promise::new(r.executor.parse().unwrap())
            .function_call(
                "execute",
                args,
                near_sdk::NearToken::from_yoctonear(0),
                Gas::from_tgas(30),
            )
            .then(self.callback(id))
    }
    /// Any observer may request reconciliation; no authority is granted by this call.
    pub fn reconcile(&self, id: u32) -> Promise {
        let c = self.pool.records.get(id as usize).expect("NotFound");
        assert!(
            matches!(
                c.status,
                pool_core::Status::Dispatched
                    | pool_core::Status::Active
                    | pool_core::Status::Unknown
            ),
            "InvalidState"
        );
        let r = &self.pool.config.recipes[c.recipe as usize];
        Promise::new(r.executor.parse().unwrap())
            .function_call(
                "status",
                near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({"id":id})).unwrap(),
                near_sdk::NearToken::from_yoctonear(0),
                Gas::from_tgas(10),
            )
            .then(self.callback(id))
    }
    #[private]
    pub fn on_result(&mut self, id: u32, #[callback_result] result: Result<String, PromiseError>) {
        let executor = self
            .pool
            .records
            .get(id as usize)
            .map(|c| self.pool.config.recipes[c.recipe as usize].executor.clone())
            .expect("NotFound");
        let status = result.unwrap_or_else(|_| "unknown".into());
        let update = match status.as_str() {
            "completed" => self.pool.settle(&executor, id, true),
            "cancelled" => self.pool.settle(&executor, id, false),
            "active" => self.pool.acknowledge(id, true),
            _ => self.pool.acknowledge(id, false),
        };
        // Failed reconciliation retains the prior encumbrance. Emit the reason.
        event(
            "reconciled",
            near_sdk::serde_json::json!({"id":id,"outcome":status,"error":update.err().map(|e|e.to_string())}),
        );
    }
    pub fn settle(&mut self, id: u32, completed: bool) {
        must(self.pool.settle(&caller(), id, completed));
        event(
            "settled",
            near_sdk::serde_json::json!({"id":id,"completed":completed}),
        );
    }
    pub fn cancel(&mut self, id: u32) {
        must(self.pool.cancel(&caller(), id, now()));
        event("cancelled", near_sdk::serde_json::json!({"id":id}));
    }
    pub fn set_paused(&mut self, paused: bool) {
        must(self.pool.set_paused(&caller(), paused));
        event("paused", near_sdk::serde_json::json!({"paused":paused}));
    }
    pub fn observe(&mut self, values: Vec<i64>) {
        must(self.pool.observe(&caller(), values, now()));
        event(
            "observed",
            near_sdk::serde_json::json!({"version":self.pool.version,"values":self.pool.values}),
        );
    }
}
impl Contract {
    fn callback(&self, id: u32) -> Promise {
        Promise::new(env::current_account_id()).function_call(
            "on_result",
            near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({"id":id})).unwrap(),
            near_sdk::NearToken::from_yoctonear(0),
            Gas::from_tgas(30),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::{test_utils::VMContextBuilder, testing_env};
    fn context(actor: &str) {
        let mut b = VMContextBuilder::new();
        b.current_account_id("pool.test.near".parse().unwrap())
            .predecessor_account_id(actor.parse().unwrap())
            .signer_account_id(actor.parse().unwrap())
            .block_timestamp(100_000_000_000);
        testing_env!(b.build());
    }
    #[test]
    fn shell_uses_predecessor_and_stores_actual_core_result() {
        context("pool.test.near");
        let mut c = Contract::new(pool_core::fixtures::config("gpu").unwrap());
        let q = Request {
            recipe: 0,
            units: 4,
            version: 0,
            expires_at: 200,
        };
        context("intruder.test.near");
        assert!(!c.submit(q.clone()).decision.allowed);
        context("research.test.near");
        assert_eq!(c.submit(q).id, Some(0));
        assert_eq!(c.get_pool().values[3], 4);
        assert_eq!(c.get_commitments(0, 100).len(), 1);
    }
}
