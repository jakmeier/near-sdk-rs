use crate::sharded_fungible_token::core::ShardedFungibleTokenCoreRoot;
use near_sdk::json_types::U128;
use near_sdk::{near, AccountId, PromiseOrValue};

#[near]
#[derive(Default)]
pub struct ShardedFungibleTokenRoot {
    total_supply: U128,
}

impl ShardedFungibleTokenCoreRoot for ShardedFungibleTokenRoot {
    fn ft_total_supply(&self) -> U128 {
        self.total_supply
    }

    fn ft_balance_of(&self, _account_id: AccountId) -> PromiseOrValue<U128> {
        // this needs a cross-contract call
        todo!()
    }
}

// TODO: implement minting etc
