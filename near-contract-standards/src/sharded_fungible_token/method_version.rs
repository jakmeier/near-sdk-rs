use near_sdk::{env, near};

const ERR_UNSUPPORTED_VERSION: &str = "Unsupported version";

/// Defines which contract version a method call was meant for.
///
/// This helps dealing with smart contracts updates, especially when using
/// global contract code that will only gradually update shard by shard.
#[derive(Clone, Copy)]
#[near(serializers = [borsh, json])]
pub struct MethodVersion {
    /// revision of the contract standard
    pub standard: u32,
    /// revision of the specific implementation
    pub contract: u32,
}

impl MethodVersion {
    pub fn v0() -> Self {
        MethodVersion { standard: 0, contract: 0 }
    }

    pub fn assert_v0(&self) {
        match (self.standard, self.standard) {
            // Initial version
            (0, 0) => (),
            // New versions may handle old and new versions but the initial
            // version can only handle (0,0).
            _else => env::panic_str(ERR_UNSUPPORTED_VERSION),
        }
    }
}
