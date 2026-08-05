use anchor_lang::prelude::*;

/// Router errors. Anchor assigns `6000 + variant index`, so this enum is
/// append-only: new variants go at the end, unused ones stay in place.
#[error_code]
pub enum ErrorCode {
    #[msg("Router is paused")]
    Paused,

    #[msg("amount_in must be greater than 0")]
    AmountInMustBeGreaterThanZero,

    #[msg("min_return must be greater than 0")]
    MinReturnMustBeGreaterThanZero,

    #[msg("No hops provided")]
    NoHops,

    #[msg("Too many hops")]
    TooManyHops,

    #[msg("Min return not reached")]
    MinReturnNotReached,

    #[msg("Invalid accounts length")]
    InvalidAccountsLength,

    #[msg("Invalid instruction data")]
    InvalidInstructionData,

    #[msg("Venue program is not in the allowlist")]
    VenueNotAllowed,

    #[msg("Invalid token account")]
    InvalidTokenAccount,

    #[msg("Source token account is not owned by the swap authority")]
    InvalidSourceTokenAccount,

    #[msg("Destination token account is not owned by the swap authority")]
    InvalidDestinationTokenAccount,

    #[msg("Hop source does not chain from the previous hop destination")]
    InvalidHopChain,

    #[msg("Invalid hop accounts")]
    InvalidHopAccounts,

    #[msg("Unexpected SA-owned token account in CPI")]
    UnexpectedSaTokenAccount,

    #[msg("Actual amount in is out of bounds")]
    InvalidActualAmountIn,

    #[msg("Amount out must be greater than 0")]
    AmountOutMustBeGreaterThanZero,

    #[msg("Invalid platform fee account")]
    InvalidFeeAccount,

    #[msg("Fee exceeds the maximum allowed")]
    FeeTooHigh,

    #[msg("Calculation error / overflow")]
    CalculationError,

    #[msg("Unauthorized")]
    Unauthorized,

    #[msg("Venue already present in allowlist")]
    VenueAlreadyPresent,

    #[msg("Venue not found in allowlist")]
    VenueNotFound,

    #[msg("Venue allowlist is full")]
    VenueListFull,

    #[msg("A native-SOL route must be exactly one hop")]
    NativeRouteMustBeSingleHop,

    /// Retired; kept so the codes after it keep their numbers.
    #[msg("Native-SOL output fee is not supported (retired)")]
    NativeFeeUnsupported,

    #[msg("Invalid hop leg kind")]
    InvalidLegKind,

    /// Retired; kept so the codes after it keep their numbers.
    #[msg("Boundary legs must be token accounts (retired)")]
    NativeBoundaryNotAllowed,

    #[msg("Native transition requires the SA's wSOL account")]
    InvalidWsolTransition,

    #[msg("A token boundary requires its user account / mint / token program")]
    MissingBoundaryAccount,

    #[msg("Boundary token account does not carry the route's boundary mint")]
    BoundaryMintMismatch,

    #[msg("Actual input exceeds max_amount_in")]
    MaxAmountInExceeded,

    #[msg("Exact-out route produced less than amount_out")]
    ExactOutNotReached,

    #[msg("Swap authority is not the payer's pooled SA PDA")]
    InvalidSwapAuthority,

    #[msg("Program cannot be allowlisted as a venue (token/system/ATA/self)")]
    VenueProgramForbidden,

    #[msg("New admin cannot be the default (zero) address")]
    InvalidAdminAddress,

    #[msg("Unknown route mode (expected 0 = exact-in, 1 = exact-out)")]
    InvalidRouteMode,
}
