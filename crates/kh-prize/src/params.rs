//! The numbers: the generated sheet constants (sheets/port/sora-prizes.json).

include!(env!("KH_PRIZE_PARAMS_RS"));

/// The sheet's values.
pub const P: PrizeParams = PRIZE_PARAMS;
