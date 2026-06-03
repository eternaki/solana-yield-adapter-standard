/// Governance registry PDA: `seeds = [REGISTRY_SEED]`.
pub const REGISTRY_SEED: &[u8] = b"registry";

/// One whitelist entry per adapter program: `seeds = [ENTRY_SEED, adapter_program]`.
pub const ENTRY_SEED: &[u8] = b"adapter_entry";

/// A user's routing position into one adapter:
/// `seeds = [POSITION_SEED, owner, adapter_program]`.
pub const POSITION_SEED: &[u8] = b"position";

/// Max human-readable adapter name length stored in an entry.
pub const MAX_NAME_LEN: usize = 32;

/// Hard cap on whitelisted adapters — bounds registry growth.
pub const MAX_ADAPTERS: u32 = 256;
