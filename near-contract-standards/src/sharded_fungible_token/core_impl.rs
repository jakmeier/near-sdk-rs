use crate::fungible_token::events::FtTransfer;
use crate::sharded_fungible_token::call_receiver::TransferCallResponseValue;
use crate::sharded_fungible_token::core::{ext_sharded_ft_core, ShardedFungibleTokenCore};
use crate::sharded_fungible_token::method_version::MethodVersion;
use near_sdk::env::{
    current_account_id, current_context, predecessor_account_id, predecessor_context,
};
use near_sdk::json_types::U128;
use near_sdk::{
    assert_one_yocto, env, near, require, AccountId, ContractContext, Gas,
    GlobalContractCodeIdentifier, Promise, PromiseOrValue, PromiseResult,
};

use super::call_receiver::ext_sharded_ft_receiver;

const ERR_BALANCE_OVERFLOW: &str = "Balance overflow";
const ERR_ACCESS_DENIED: &str = "Access denied";
const ERR_SHARDED_CONTEXT: &str = "Must be sharded context";

// TODO: Requires TIGHT gas estimation
// TODO(optional): Make it possible to update the gas value somehow
const TRANSFER_REFUND_GAS: Gas = Gas::from_tgas(1);
const RESOLVE_CALL_RECEIVE_GAS: Gas = Gas::from_tgas(1);
const CALL_REFUND_GAS: Gas = Gas::from_tgas(1);

pub type Balance = u128;

/// Implementation of a sharded fungible token standard.
#[near]
#[derive(Default)]
pub struct ShardedFungibleToken {
    /// Local account balance.
    ///
    /// This is how much of the FT the current account holds. It starts at 0 and
    /// can only be increased by incoming deposits.
    ///
    /// This basic core implementation only allows minting once at
    /// initialization, only on one account, using `new_with_initial_supply`.
    ///
    /// By deploying this contract under a **sharded contract context**, this is
    /// protected from modification outside of the code provided by the holder
    /// of the global contract account id.
    pub balance: Balance,
}

impl ShardedFungibleToken {
    pub fn new() -> Self {
        Self { balance: 0 }
    }

    pub fn new_with_initial_supply(initial_supply: U128) -> Self {
        // Check we are the "admin" instance, defined as `ft.near@ft.near`
        let my_context = current_context();
        let sender_context = predecessor_context();
        let sender_account = predecessor_account_id();
        let admin_context = ContractContext::Sharded {
            code_id: GlobalContractCodeIdentifier::AccountId(sender_account),
        };
        require!(matches!(sender_context, ContractContext::Root), ERR_ACCESS_DENIED);
        require!(my_context == admin_context, ERR_ACCESS_DENIED);
        Self { balance: initial_supply.into() }
    }

    pub fn internal_deposit(&mut self, amount: Balance) {
        self.balance = self
            .balance
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str(ERR_BALANCE_OVERFLOW));
    }

    pub fn internal_withdraw(&mut self, amount: Balance) {
        self.balance = self
            .balance
            .checked_sub(amount)
            .unwrap_or_else(|| env::panic_str(ERR_BALANCE_OVERFLOW));
    }
}

impl ShardedFungibleTokenCore for ShardedFungibleToken {
    fn balance(&self, version: MethodVersion) -> U128 {
        version.assert_v0();
        self.balance.into()
    }

    fn sharded_ft_transfer(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        version: MethodVersion,
    ) {
        version.assert_v0();

        // Ensure the call is from the owner
        let me = env::current_account_id();
        let sender_id = env::predecessor_account_id();
        require!(me == sender_id, ERR_ACCESS_DENIED);

        // Ensure caller has full access
        assert_one_yocto();

        // Get rid of a corner case that's not needed.
        require!(sender_id != receiver_id, "Sender and receiver should be different");

        self.internal_withdraw(amount.into());

        // Defensive check that we are in a sharded context.
        // This contract was built for a sharded context. Using it in other
        // contexts may have unintended consequences.
        let my_context = current_context();
        require!(matches!(my_context, ContractContext::Sharded { .. }), ERR_SHARDED_CONTEXT);

        ext_sharded_ft_core::ext(receiver_id)
            .with_contract_context(my_context.clone())
            .with_unused_gas_weight(1)
            .sharded_ft_receive(amount, memo, MethodVersion::v0())
            .then(
                ext_sharded_ft_core::ext(me)
                    .with_contract_context(my_context)
                    .with_static_gas(TRANSFER_REFUND_GAS)
                    .settle_sharded_ft_transfer(amount, MethodVersion::v0()),
            );
    }

    fn sharded_ft_receive(&mut self, amount: U128, memo: Option<String>, version: MethodVersion) {
        require_internal_sharded_call();
        version.assert_v0();

        self.internal_deposit(amount.into());

        // This is the point of no return for a sharded_ft_transfer, so we emit
        // the transfer here.
        FtTransfer {
            old_owner_id: &predecessor_account_id(),
            new_owner_id: &current_account_id(),
            amount,
            memo: memo.as_deref(),
        }
        .emit();
    }

    fn settle_sharded_ft_transfer(&mut self, amount: U128, version: MethodVersion) {
        require_private();
        require_internal_sharded_call();

        // Settlements are only sent as callbacks, hence from self.
        // Since this is version 0 and we should only ever get calls from oursel
        version.assert_v0();

        let amount: Balance = amount.into();

        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                // ok => nothing to do
            }
            PromiseResult::Failed => {
                // transfer failed, take back the amount sent
                self.internal_deposit(amount);
            }
        };
    }

    fn sharded_ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        target_contract_context: ContractContext,
        amount: U128,
        memo: Option<String>,
        msg: String,
        version: MethodVersion,
    ) -> PromiseOrValue<U128> {
        version.assert_v0();
        assert_one_yocto();

        let sender_id = env::predecessor_account_id();

        // Defensive check that we are in a sharded context.
        // This contract was built for a sharded context. Using it in other
        // contexts may have unintended consequences.
        let sharded_ft_context = current_context();
        require!(
            matches!(sharded_ft_context, ContractContext::Sharded { .. }),
            ERR_SHARDED_CONTEXT
        );

        // remove tokens from local balance
        // floating = amount
        self.internal_withdraw(amount.into());

        let my_account_id = env::current_account_id();

        // This can fail. If it does, callbacks are ready to refund without loss.
        // The return value SHOULD be a `TransferCallResponseValue`, either
        // directly or through a further chain of promises. If something else
        // returned, it is treated like a failure. (Refund all, transfer nothing.)
        let on_transfer: Promise = ext_sharded_ft_receiver::ext(receiver_id.clone())
            .with_contract_context(target_contract_context)
            .with_unused_gas_weight(1)
            .sharded_ft_on_transfer(sender_id.clone(), amount, msg, MethodVersion::v0());

        // this must not fail, or tokens are burnt rather than received
        let deposit: Promise = ext_sharded_ft_core::ext(receiver_id.clone())
            .with_contract_context(sharded_ft_context.clone())
            .with_static_gas(RESOLVE_CALL_RECEIVE_GAS)
            .sharded_ft_resolve_call(amount, memo, MethodVersion::v0())
            .into();
        // this must not fail, or tokens are burnt burn rather than refunded
        let refund: Promise = ext_sharded_ft_core::ext(my_account_id.clone())
            .with_contract_context(sharded_ft_context.clone())
            .with_static_gas(CALL_REFUND_GAS)
            .sharded_ft_refund_call(amount, MethodVersion::v0())
            .into();

        // Both calls will have the same input and run the same check.
        // If both have the same version, this ensures either both are reject
        // or accept the transfer.
        // When introducing update that change the return value, take extra care
        // to not have a situation where an old version will refund and another
        // will transfer (or vice versa). This might require code that can
        // handle multiple versions.

        on_transfer.then_many(&[&deposit, &refund]);

        // The final value returned should be how much was transferred, to be
        // in line with NEP-141.
        deposit.into()
    }

    fn sharded_ft_resolve_call(
        &mut self,
        amount: U128,
        memo: Option<String>,
        version: MethodVersion,
    ) -> U128 {
        require_private();
        require_internal_sharded_call();

        // Defensive assumption: Don't refund anything from a potentially
        // incompatible version. Updates might need to be done incrementally,
        // removing this check first and waiting until it has been distributed
        // globally to avoid the risk of burn tokens.
        version.assert_v0();

        let transfer_amount = TransferCallResponseValue::validated_used_amount(amount);

        self.internal_deposit(transfer_amount.0);
        FtTransfer {
            old_owner_id: &env::predecessor_account_id(),
            new_owner_id: &env::current_account_id(),
            amount: transfer_amount,
            memo: memo.as_deref(),
        }
        .emit();

        transfer_amount
    }

    fn sharded_ft_refund_call(&mut self, amount: U128, version: MethodVersion) {
        require_private();
        require_internal_sharded_call();

        // Defensive assumption: Don't refund anything from a potentially
        // incompatible version. Updates might need to be done incrementally,
        // removing this check first and waiting until it has been distributed
        // globally to avoid the risk of burn tokens.
        version.assert_v0();

        let unused_amount = TransferCallResponseValue::validated_unused_amount(amount);

        self.internal_deposit(unused_amount.0);
    }
}

/// Checks that the caller is from the same sharded context and panics if this
/// is not satisfied.
fn require_internal_sharded_call() {
    // Check we are called by the exact same context,
    let my_context = current_context();
    let sender_context = predecessor_context();
    require!(my_context == sender_context, ERR_ACCESS_DENIED);

    // Defensive check that we are in a sharded context.
    // This contract was built for a sharded context. Using it in other
    // contexts may have unintended consequences.
    require!(matches!(my_context, ContractContext::Sharded { .. }), ERR_SHARDED_CONTEXT);
}

fn require_private() {
    require!(
        near_sdk::env::current_account_id() == near_sdk::env::predecessor_account_id(),
        ERR_ACCESS_DENIED
    );
}

impl TransferCallResponseValue {
    fn read_and_validate(amount: U128) -> Option<Self> {
        match env::promise_result(0) {
            PromiseResult::Successful(value) => {
                if let Ok(response) =
                    near_sdk::serde_json::from_slice::<TransferCallResponseValue>(&value)
                {
                    // The call was successful, expected to read how much to
                    // transfer and how much to keep on the receiver.
                    // However, we can't trust the value returned by
                    // sharded_ft_on_transfer, we must verify that it adds up to
                    // value.
                    let expected_amount = response.unused.0.checked_add(response.used.0)?;
                    if amount.0 != expected_amount {
                        Some(response)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            PromiseResult::Failed => None,
        }
    }

    fn validated_unused_amount(amount: U128) -> U128 {
        match Self::read_and_validate(amount) {
            Some(response) => response.unused,
            // invalid input, refunding the full amount
            None => amount,
        }
    }

    fn validated_used_amount(amount: U128) -> U128 {
        match Self::read_and_validate(amount) {
            Some(response) => response.unused,
            // invalid input, refunding the full amount
            None => 0.into(),
        }
    }
}
