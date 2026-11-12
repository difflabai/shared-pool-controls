//! Thin NEAR shell: identity, storage, receipts, and asynchronous execution.
use near_sdk::{env, near, require, AccountId, Gas, NearToken, PanicOnDefault};
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
    pub fn dispatch(&mut self, id: u32) {
        must(self.pool.dispatch(&caller(), id, now()));
        let c = &self.pool.records[id as usize];
        let r = &self.pool.config.recipes[c.recipe as usize];
        let args = near_sdk::serde_json::to_vec(
            &near_sdk::serde_json::json!({"id":id,"recipe":c.recipe,"units":c.units}),
        )
        .unwrap();
        call_then_callback(&r.executor, "execute", &args, Gas::from_tgas(30), id);
    }
    /// Any observer may request reconciliation; no authority is granted by this call.
    pub fn reconcile(&self, id: u32) {
        // NT-F2: near-sdk generates the nonpayable check only for &mut self methods.
        require!(
            env::attached_deposit().is_zero(),
            "Method reconcile doesn't accept deposit"
        );
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
        let args = near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({"id":id})).unwrap();
        call_then_callback(&r.executor, "status", &args, Gas::from_tgas(10), id);
    }
    #[private]
    pub fn on_result(&mut self, id: u32) {
        // NT-F4: read the raw result. A failed call, a missing result, more than
        // 64 bytes, or anything but one JSON string is "unknown". No promise result
        // and no id makes this callback panic.
        let status = if env::promise_results_count() == 1 {
            env::promise_result_checked(0, 64)
                .ok()
                .and_then(|b| near_sdk::serde_json::from_slice::<String>(&b).ok())
        } else {
            None
        }
        .unwrap_or_else(|| "unknown".into());
        let Ok(c) = self.pool.record(id) else {
            // No such record: nothing to update, so log the result and return.
            return event(
                "reconciled",
                near_sdk::serde_json::json!({"id":id,"outcome":status,"error":"NotFound"}),
            );
        };
        let executor = self.pool.config.recipes[c.recipe as usize].executor.clone();
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
/// Calls `method` on the executor with no deposit, then this contract's
/// `on_result({id})` with 30 Tgas, and makes the callback's result the result
/// of the current call. NT-F3: this uses near-sdk's low-level promise functions.
/// Its `Promise` type links the host functions for every action (deploy, keys,
/// transfer, stake, accounts); build.sh fails if any of them is imported.
fn call_then_callback(executor: &str, method: &str, args: &[u8], gas: Gas, id: u32) {
    let zero = NearToken::from_yoctonear(0);
    let call = env::promise_create(executor.parse().unwrap(), method, args, zero, gas);
    let callback = env::promise_then(
        call,
        env::current_account_id(),
        "on_result",
        near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({"id":id})).unwrap(),
        zero,
        Gas::from_tgas(30),
    );
    env::promise_return(callback);
}
#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::mock::MockAction;
    use near_sdk::serde_json::{json, Value};
    use near_sdk::test_utils::{get_created_receipts, get_logs, VMContextBuilder};
    use near_sdk::{testing_env, NearToken, PromiseResult};
    use pool_core::Status;
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
    /// The pool calling its own callback, with these promise results.
    fn callback_context(results: Vec<PromiseResult>) {
        let mut b = VMContextBuilder::new();
        b.current_account_id("pool.test.near".parse().unwrap())
            .predecessor_account_id("pool.test.near".parse().unwrap())
            .block_timestamp(100_000_000_000);
        testing_env!(
            b.build(),
            near_sdk::test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            Default::default(),
            results
        );
    }
    /// The gpu pool with record 0 (recipe 0, 4 units) Dispatched.
    fn dispatched() -> Contract {
        context("pool.test.near");
        let mut c = Contract::new(pool_core::fixtures::config("gpu").unwrap());
        context("research.test.near");
        let q = Request {
            recipe: 0,
            units: 4,
            version: 0,
            expires_at: 200,
        };
        assert_eq!(c.submit(q).id, Some(0));
        c.dispatch(0);
        assert_eq!(c.get_pool().records[0].status, Status::Dispatched);
        c
    }
    /// The single event logged by the last call.
    fn last_event() -> near_sdk::serde_json::Value {
        let logs = get_logs();
        assert_eq!(logs.len(), 1, "{logs:?}");
        near_sdk::serde_json::from_str(logs[0].strip_prefix("EVENT_JSON:").unwrap()).unwrap()
    }
    /// A function call: (method, JSON args, deposit in yocto, gas in Tgas, unused-gas weight).
    type Call = (String, Value, u128, u64, u64);
    /// Every receipt created by the last call: (receiver, receipts it waits for, calls).
    fn receipts() -> Vec<(String, Vec<u64>, Vec<Call>)> {
        get_created_receipts()
            .into_iter()
            .map(|r| {
                let calls = r.actions.into_iter().map(|a| match a {
                    MockAction::FunctionCallWeight {
                        method_name,
                        args,
                        attached_deposit,
                        prepaid_gas,
                        gas_weight,
                        ..
                    } => (
                        String::from_utf8(method_name).unwrap(),
                        near_sdk::serde_json::from_slice(&args).unwrap(),
                        attached_deposit.as_yoctonear(),
                        prepaid_gas.as_tgas(),
                        gas_weight.0,
                    ),
                    other => panic!("not a function call: {other:?}"),
                });
                (
                    r.receiver_id.to_string(),
                    r.receipt_indices,
                    calls.collect(),
                )
            })
            .collect()
    }
    #[test]
    fn dispatch_and_reconcile_call_the_executor_then_on_result() {
        let call = |method: &str, args: Value, tgas: u64| (method.to_string(), args, 0, tgas, 0);
        let on_result = (
            "pool.test.near".to_string(),
            vec![0],
            vec![call("on_result", json!({"id":0}), 30)],
        );
        let c = dispatched();
        let execute = call("execute", json!({"id":0,"recipe":0,"units":4}), 30);
        assert_eq!(
            receipts(),
            vec![
                ("executor.test.near".into(), vec![], vec![execute]),
                on_result.clone()
            ]
        );
        context("anyone.test.near");
        c.reconcile(0);
        let status = call("status", json!({"id":0}), 10);
        assert_eq!(
            receipts(),
            vec![
                ("executor.test.near".into(), vec![], vec![status]),
                on_result
            ]
        );
    }
    #[test]
    #[should_panic(expected = "Method reconcile doesn't accept deposit")]
    fn nt_f2_reconcile_rejects_deposit() {
        let c = dispatched();
        let mut b = VMContextBuilder::new();
        b.current_account_id("pool.test.near".parse().unwrap())
            .predecessor_account_id("anyone.test.near".parse().unwrap())
            .attached_deposit(NearToken::from_yoctonear(1));
        testing_env!(b.build());
        c.reconcile(0);
    }
    #[test]
    fn reconcile_without_deposit_is_open_to_anyone() {
        let c = dispatched();
        context("anyone.test.near");
        c.reconcile(0);
    }
    #[test]
    fn nt_f4_non_string_result_becomes_unknown() {
        use Status::{Active, Cancelled, Completed, Unknown};
        let ok = |b: &[u8]| vec![PromiseResult::Successful(b.to_vec())];
        // "completed" as a JSON string padded with trailing whitespace to n bytes.
        let padded = |n: usize| {
            let mut s = b"\"completed\"".to_vec();
            s.resize(n, b' ');
            s
        };
        let long = format!("\"{}\"", "a".repeat(10_000));
        // (case, promise results, record status afterwards, logged outcome)
        let cases: Vec<(&str, Vec<PromiseResult>, Status, &str)> = vec![
            ("number", ok(b"5"), Unknown, "unknown"),
            ("object", ok(br#"{"status":"active"}"#), Unknown, "unknown"),
            ("array", ok(br#"["active"]"#), Unknown, "unknown"),
            ("null", ok(b"null"), Unknown, "unknown"),
            ("empty", ok(b""), Unknown, "unknown"),
            ("unquoted", ok(b"completed"), Unknown, "unknown"),
            ("invalid UTF-8", ok(b"\"\xff\xfe\""), Unknown, "unknown"),
            ("10 kB string", ok(long.as_bytes()), Unknown, "unknown"),
            ("65 bytes", ok(&padded(65)), Unknown, "unknown"),
            ("failed", vec![PromiseResult::Failed], Unknown, "unknown"),
            ("no result", vec![], Unknown, "unknown"),
            (
                "two results",
                vec![
                    PromiseResult::Successful(b"\"active\"".to_vec()),
                    PromiseResult::Successful(b"\"active\"".to_vec()),
                ],
                Unknown,
                "unknown",
            ),
            ("other string", ok(b"\"bogus\""), Unknown, "bogus"),
            ("active", ok(b"\"active\""), Active, "active"),
            ("64 bytes", ok(&padded(64)), Completed, "completed"),
            ("cancelled", ok(b"\"cancelled\""), Cancelled, "cancelled"),
        ];
        for (name, results, status, outcome) in cases {
            let mut c = dispatched();
            callback_context(results);
            c.on_result(0);
            let p = c.get_pool();
            assert_eq!(p.records[0].status, status, "{name}");
            // Only a definitive cancellation releases the 4 reserved units.
            let held = if status == Cancelled { 0 } else { 4 };
            assert_eq!(p.values[3], held, "{name}");
            let e = last_event();
            assert_eq!(e["event"], "reconciled", "{name}");
            assert_eq!(e["data"][0]["outcome"], outcome, "{name}");
            assert!(e["data"][0]["error"].is_null(), "{name}");
        }
    }
    #[test]
    fn nt_f4_result_for_unknown_id_is_logged() {
        let mut c = dispatched();
        let before = c.get_pool();
        for id in [1, 7, u32::MAX] {
            callback_context(vec![PromiseResult::Successful(b"\"completed\"".to_vec())]);
            c.on_result(id);
            assert_eq!(c.get_pool(), before);
            assert_eq!(
                last_event()["data"][0],
                near_sdk::serde_json::json!({"id":id,"outcome":"completed","error":"NotFound"})
            );
        }
    }
}
