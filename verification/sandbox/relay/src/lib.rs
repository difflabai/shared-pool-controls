//! Sandbox helper contract (about 60 lines), deployed under several accounts.
//!
//! RT-PRED: `forward` relays a call to the pool. The account that signs
//! `forward` is the relay's caller, but the pool sees the relay's account as
//! predecessor (ASM-PRED), so a forwarded submit is attributed to the relay.
//!
//! RT-RACE: deployed as a recipe's executor. With `settle = Some(c)`,
//! `execute` and `status` first notify the caller (the pool) with
//! `settle(id, c)` from inside their own receipt, then return "active".
//! NEAR emits the receipts a call creates before the data receipt that
//! carries its return value, so on one shard the settle lands before the
//! pool's `on_result` callback, deterministically: the callback's "active" is
//! a late ack. With `settle = None` they only return "active".
use near_sdk::{env, near, AccountId, Gas, GasWeight, NearToken, Promise};

#[near(contract_state)]
#[derive(Default)]
pub struct Relay {
    /// What `execute` and `status` report through `settle` first (RT-RACE).
    settle: Option<bool>,
}

#[near]
impl Relay {
    /// Calls `receiver.method(args)` (JSON text) with all remaining gas and
    /// returns that call's result, so the transaction fails if it fails.
    pub fn forward(&self, receiver: AccountId, method: String, args: String) -> Promise {
        Promise::new(receiver).function_call_weight(
            method,
            args.into_bytes(),
            NearToken::from_yoctonear(0),
            Gas::from_gas(0),
            GasWeight(1),
        )
    }
    pub fn set_settle(&mut self, settle: Option<bool>) {
        self.settle = settle;
    }
    /// The pool's `execute` call.
    pub fn execute(&self, id: u32, recipe: u32, units: u32) -> String {
        let _ = (recipe, units);
        self.answer(id)
    }
    /// The pool's `reconcile` read.
    pub fn status(&self, id: u32) -> String {
        self.answer(id)
    }
}

impl Relay {
    /// Settle first (a detached promise to the caller), then "active".
    fn answer(&self, id: u32) -> String {
        if let Some(completed) = self.settle {
            let args = format!("{{\"id\":{id},\"completed\":{completed}}}");
            Promise::new(env::predecessor_account_id())
                .function_call_weight(
                    "settle".to_string(),
                    args.into_bytes(),
                    NearToken::from_yoctonear(0),
                    Gas::from_gas(0),
                    GasWeight(1),
                )
                .detach();
        }
        "active".to_string()
    }
}
