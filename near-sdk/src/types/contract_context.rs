// TODO: These types might be defined in a dependency for primitives

use near_account_id::AccountId;
use near_sdk_macros::near;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[near(inside_nearsdk, serializers=[borsh, json])]

pub enum ContractContext {
    Root,
    Sharded { code_id: GlobalContractCodeIdentifier },
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[near(inside_nearsdk, serializers=[borsh, json])]
pub enum GlobalContractCodeIdentifier {
    CodeHash(String),
    AccountId(AccountId),
}

pub enum ContextPermissions {
    // TODO
}
