pub mod adapter_current_value;
pub mod adapter_deposit;
pub mod adapter_initialize;
pub mod adapter_withdraw;

// Glob re-export so Anchor's `#[program]` macro resolves the generated
// account-context modules. The benign `handler` name ambiguity is expected
// (lib.rs always calls handlers via their full module path).
pub use adapter_current_value::*;
pub use adapter_deposit::*;
pub use adapter_initialize::*;
pub use adapter_withdraw::*;
