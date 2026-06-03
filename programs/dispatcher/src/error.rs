use anchor_lang::prelude::*;

#[error_code]
pub enum DispatcherError {
    /// 6000 — signer is not the registry admin.
    #[msg("Unauthorized: not the registry admin")]
    Unauthorized,
    /// 6001 — adapter is not whitelisted or has been paused.
    #[msg("Adapter is not active in the registry")]
    AdapterInactive,
    /// 6002 — adapter interface version differs from this dispatcher's.
    #[msg("Adapter interface version mismatch")]
    VersionMismatch,
    /// 6003 — registry has reached MAX_ADAPTERS.
    #[msg("Adapter registry is full")]
    RegistryFull,
    /// 6004 — name exceeds MAX_NAME_LEN.
    #[msg("Adapter name too long")]
    NameTooLong,
    /// 6005 — amount or shares is zero.
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    /// 6006 — checked arithmetic overflowed.
    #[msg("Arithmetic overflow")]
    MathOverflow,
    /// 6007 — adapter program account is not executable.
    #[msg("Adapter program is not executable")]
    NotExecutable,
    /// 6008 — the routed adapter did not return a value via set_return_data.
    #[msg("Adapter returned no value")]
    MissingReturnData,
    /// 6009 — the entry/position references a different adapter than supplied.
    #[msg("Adapter program mismatch")]
    AdapterMismatch,
    /// 6010 — value reported by the adapter is staler than allowed.
    #[msg("Adapter value is stale")]
    StaleValue,
}
