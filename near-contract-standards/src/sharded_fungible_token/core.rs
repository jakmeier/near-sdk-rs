use near_sdk::ext_contract;
use near_sdk::json_types::U128;
use near_sdk::AccountId;
use near_sdk::ContractContext;
use near_sdk::PromiseOrValue;

use super::method_version::MethodVersion;
/// The core methods for a basic sharded fungible token. Extension standards may
/// be added in addition to this trait.
///
/// This is the contract to be deployed globally and used by all users on their
/// accounts.
#[ext_contract(ext_sharded_ft_core)]
pub trait ShardedFungibleTokenCore {
    /// Read and return the balance of the current account.
    ///
    /// Cannot read the balance of other accounts. Go to the root FT contract
    /// and use `ShardedFungibleTokenCoreRoot` to read any balance. (Not
    /// available as view call.)
    fn balance(&self, version: MethodVersion) -> U128;

    fn sharded_ft_transfer(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        version: MethodVersion,
    );

    fn sharded_ft_receive(&mut self, amount: U128, memo: Option<String>, version: MethodVersion);

    /// Called at the end of the "transfer -> receive" promise chain.
    ///
    /// This method must only ever be called as a result from
    /// sharded_ft_receive.
    ///
    /// On failure, the amount that should have been sent is returned and must
    /// be added back to the balance of the sender.
    ///
    /// Calls that are not from a sharded context that exactly matches the
    /// current sharded context must be refused.
    fn settle_sharded_ft_transfer(&mut self, amount: U128, version: MethodVersion);

    fn sharded_ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        target_contract_context: ContractContext,
        amount: U128,
        memo: Option<String>,
        msg: String,
        version: MethodVersion,
    ) -> PromiseOrValue<U128>;

    /// Callback at the end of transfer_call, on the receiver.
    fn sharded_ft_resolve_call(
        &mut self,
        amount: U128,
        memo: Option<String>,
        version: MethodVersion,
    ) -> U128;

    /// Callback at the end of transfer_call, on the sender.
    fn sharded_ft_refund_call(&mut self, amount: U128, version: MethodVersion);
}

/// This root contract interface is to be deployed on the central account that
/// manages the sharded FT. Extension standards may be added in addition to this
/// trait.
#[ext_contract(ext_sharded_ft_core_root)]
pub trait ShardedFungibleTokenCoreRoot {
    /// Returns the total supply of the token in a decimal string representation.
    fn ft_total_supply(&self) -> U128;
    /// Returns the balance of the account. If the account doesn't exist must returns `"0"`.
    fn ft_balance_of(&self, _account_id: AccountId) -> PromiseOrValue<U128>;
}
