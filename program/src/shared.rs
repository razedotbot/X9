//! Wire types and well-known program ids. The borsh layout of the types below
//! is the on-chain/off-chain contract; a client may depend on this crate or keep
//! a hand-written mirror, so the sizes are pinned by tests at the bottom.

use anchor_lang::prelude::{borsh, AnchorDeserialize, AnchorSerialize};

pub mod spl_token {
    use anchor_lang::prelude::*;
    declare_id!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
}

pub mod token_2022 {
    use anchor_lang::prelude::*;
    declare_id!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
}

pub mod system_program {
    use anchor_lang::prelude::*;
    declare_id!("11111111111111111111111111111111");
}

pub mod wsol {
    use anchor_lang::prelude::*;
    declare_id!("So11111111111111111111111111111111111111112");
}

/// One CPI account reference for a hop.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct HopAccountMeta {
    /// Index into `remaining_accounts`.
    pub index: u8,
    /// bit0 = writable, bit1 = signer.
    pub flags: u8,
}

impl HopAccountMeta {
    pub const WRITABLE: u8 = 0b01;
    pub const SIGNER: u8 = 0b10;

    pub fn new(index: u8, writable: bool, signer: bool) -> Self {
        let mut flags = 0;
        if writable {
            flags |= Self::WRITABLE;
        }
        if signer {
            flags |= Self::SIGNER;
        }
        Self { index, flags }
    }

    #[inline]
    pub fn is_writable(&self) -> bool {
        self.flags & Self::WRITABLE != 0
    }

    #[inline]
    pub fn is_signer(&self) -> bool {
        self.flags & Self::SIGNER != 0
    }
}

/// The per-hop shape [`crate::hop::execute_hop`] consumes; the wire carries
/// [`HopMetaV2`] and the leg kinds reach `execute_hop` as separate arguments.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct HopMeta {
    /// Index into `remaining_accounts` of the venue program to invoke.
    pub program_id_index: u8,
    /// Index of this hop's input leg.
    pub source_token_index: u8,
    /// Index of this hop's output leg.
    pub destination_token_index: u8,
    /// Offset in `data` where the running input amount (u64 LE) is spliced;
    /// `-1` = use `data` verbatim.
    pub amount_in_offset: i16,
    /// CPI account list, in the order the venue expects.
    pub accounts: Vec<HopAccountMeta>,
    /// Pre-encoded venue instruction data.
    pub data: Vec<u8>,
}

/// Leg kind values for [`HopMetaV2::source_kind`] / [`HopMetaV2::dest_kind`].
pub const LEG_TOKEN: u8 = 0;
pub const LEG_NATIVE: u8 = 1;

/// One swap hop with explicit leg kinds. A `LEG_NATIVE` leg is the SA PDA's own
/// lamports; the program wraps/unwraps its wSOL ATA at a transition between hops.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct HopMetaV2 {
    /// Index into `remaining_accounts` of the venue program to invoke.
    pub program_id_index: u8,
    /// Index of this hop's input leg.
    pub source_token_index: u8,
    /// Index of this hop's output leg.
    pub destination_token_index: u8,
    /// Offset in `data` where the running input amount is spliced; `-1` = none.
    pub amount_in_offset: i16,
    /// XOR mask applied byte-wise to the spliced amount; all-zero = plain splice.
    pub splice_xor_mask: [u8; 8],
    /// `LEG_TOKEN` | `LEG_NATIVE` for the source leg.
    pub source_kind: u8,
    /// `LEG_TOKEN` | `LEG_NATIVE` for the destination leg.
    pub dest_kind: u8,
    /// CPI account list, in the order the venue expects.
    pub accounts: Vec<HopAccountMeta>,
    /// Pre-encoded venue instruction data.
    pub data: Vec<u8>,
}

/// Arguments to `route_universal`. Boundary leg kinds come from the first hop's
/// `source_kind` and the last hop's `dest_kind`; `min_return` applies to the net.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct RouteArgsV2 {
    pub amount_in: u64,
    pub min_return: u64,
    pub hops: Vec<HopMetaV2>,
    pub fee_bps: u16,
    pub fee_on_input: bool,
    /// Caller-computed native top-up moved payer→SA before the hops run and
    /// swept back after; also widens the native-leg input bound. 0 = skip.
    pub sa_native_topup: u64,
}

/// Arguments to `route_exact_out` — a single-hop swap that pins the output.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct RouteArgsExactOut {
    /// Output the route must deliver, as a floor.
    pub amount_out: u64,
    /// Maximum input the route may consume; the remainder is refunded.
    pub max_amount_in: u64,
    /// The single venue hop, with its exact-out instruction pre-encoded.
    pub hops: Vec<HopMetaV2>,
    /// Per-route platform fee in bps (≤ `MAX_PLATFORM_FEE_BPS`).
    pub fee_bps: u16,
    /// true ⇒ fee on the consumed input; false ⇒ fee on the output.
    pub fee_on_input: bool,
    /// See [`RouteArgsV2::sa_native_topup`]; also sets the native deposit.
    pub sa_native_topup: u64,
}

/// [`RouteArgsUnified::mode`]: `amount_a` = `amount_in`, `amount_b` = `min_return`.
pub const ROUTE_MODE_EXACT_IN: u8 = 0;
/// [`RouteArgsUnified::mode`]: `amount_a` = `amount_out`, `amount_b` = `max_amount_in`.
pub const ROUTE_MODE_EXACT_OUT: u8 = 1;

/// Arguments to `route_unified`: one envelope for both directions, selected by
/// the in-payload `mode` byte. The amount fields are named opaquely because they
/// mean different things per mode; the mapping lives only in [`Self::split`].
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct RouteArgsUnified {
    /// [`ROUTE_MODE_EXACT_IN`] | [`ROUTE_MODE_EXACT_OUT`].
    pub mode: u8,
    /// exact-in: `amount_in` · exact-out: `amount_out`.
    pub amount_a: u64,
    /// exact-in: `min_return` · exact-out: `max_amount_in`.
    pub amount_b: u64,
    pub hops: Vec<HopMetaV2>,
    pub fee_bps: u16,
    pub fee_on_input: bool,
    /// See [`RouteArgsV2::sa_native_topup`].
    pub sa_native_topup: u64,
}

/// The direction a [`RouteArgsUnified`] resolved to, carrying the legacy args
/// the corresponding handler already takes.
pub enum RouteMode {
    ExactIn(RouteArgsV2),
    ExactOut(RouteArgsExactOut),
}

impl RouteArgsUnified {
    /// Resolve `mode` into the legacy args; `None` for an unknown mode.
    pub fn split(self) -> Option<RouteMode> {
        let Self {
            mode,
            amount_a,
            amount_b,
            hops,
            fee_bps,
            fee_on_input,
            sa_native_topup,
        } = self;
        match mode {
            ROUTE_MODE_EXACT_IN => Some(RouteMode::ExactIn(RouteArgsV2 {
                amount_in: amount_a,
                min_return: amount_b,
                hops,
                fee_bps,
                fee_on_input,
                sa_native_topup,
            })),
            ROUTE_MODE_EXACT_OUT => Some(RouteMode::ExactOut(RouteArgsExactOut {
                amount_out: amount_a,
                max_amount_in: amount_b,
                hops,
                fee_bps,
                fee_on_input,
                sa_native_topup,
            })),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::{AnchorDeserialize, AnchorSerialize};

    /// The writable / signer bits round-trip through `flags`.
    #[test]
    fn account_meta_flags() {
        let m = HopAccountMeta::new(7, true, false);
        assert_eq!(m.index, 7);
        assert!(m.is_writable() && !m.is_signer());
        let s = HopAccountMeta::new(0, false, true);
        assert!(!s.is_writable() && s.is_signer());
        let rw = HopAccountMeta::new(3, true, true);
        assert!(rw.is_writable() && rw.is_signer());
    }

    fn unified(mode: u8, a: u64, b: u64) -> RouteArgsUnified {
        RouteArgsUnified {
            mode,
            amount_a: a,
            amount_b: b,
            hops: vec![],
            fee_bps: 25,
            fee_on_input: true,
            sa_native_topup: 7,
        }
    }

    /// Each mode maps `amount_a` / `amount_b` onto its own pair of fields.
    #[test]
    fn unified_mode_maps_amounts_to_the_right_fields() {
        match unified(ROUTE_MODE_EXACT_IN, 1_000, 950).split().unwrap() {
            RouteMode::ExactIn(v2) => {
                assert_eq!(v2.amount_in, 1_000, "amount_a must be amount_in");
                assert_eq!(v2.min_return, 950, "amount_b must be min_return");
                assert_eq!((v2.fee_bps, v2.fee_on_input, v2.sa_native_topup), (25, true, 7));
            }
            RouteMode::ExactOut(_) => panic!("mode 0 must resolve to exact-in"),
        }
        match unified(ROUTE_MODE_EXACT_OUT, 1_000, 950).split().unwrap() {
            RouteMode::ExactOut(eo) => {
                assert_eq!(eo.amount_out, 1_000, "amount_a must be amount_out");
                assert_eq!(eo.max_amount_in, 950, "amount_b must be max_amount_in");
                assert_eq!((eo.fee_bps, eo.fee_on_input, eo.sa_native_topup), (25, true, 7));
            }
            RouteMode::ExactIn(_) => panic!("mode 1 must resolve to exact-out"),
        }
    }

    /// An unknown mode resolves to nothing rather than to a direction.
    #[test]
    fn unified_rejects_unknown_mode() {
        for bad in [2u8, 3, 0xFF] {
            assert!(unified(bad, 1, 1).split().is_none(), "mode {bad} must be rejected");
        }
    }

    /// The unified envelope is 32 B — one `mode` byte over the legacy 31.
    #[test]
    fn unified_wire_envelope_is_pinned() {
        assert_eq!(unified(ROUTE_MODE_EXACT_IN, 1, 2).try_to_vec().unwrap().len(), 32);
        assert_eq!(unified(ROUTE_MODE_EXACT_OUT, 1, 2).try_to_vec().unwrap().len(), 32);
        let v2 = RouteArgsV2 {
            amount_in: 1,
            min_return: 2,
            hops: vec![],
            fee_bps: 25,
            fee_on_input: true,
            sa_native_topup: 7,
        };
        assert_eq!(v2.try_to_vec().unwrap().len() + 1, 32);
    }

    /// The unified args round-trip, with `mode` as the first byte.
    #[test]
    fn unified_borsh_roundtrip() {
        let args = RouteArgsUnified {
            mode: ROUTE_MODE_EXACT_OUT,
            amount_a: 5_000_000,
            amount_b: 4_900_000,
            hops: vec![HopMetaV2 {
                program_id_index: 0,
                source_token_index: 1,
                destination_token_index: 2,
                amount_in_offset: -1,
                splice_xor_mask: [0; 8],
                source_kind: LEG_NATIVE,
                dest_kind: LEG_TOKEN,
                accounts: vec![HopAccountMeta::new(1, true, false)],
                data: vec![0xAB; 24],
            }],
            fee_bps: 300,
            fee_on_input: false,
            sa_native_topup: 2_100_000,
        };
        let bytes = args.try_to_vec().unwrap();
        assert_eq!(RouteArgsUnified::try_from_slice(&bytes).unwrap(), args);
        assert_eq!(bytes[0], ROUTE_MODE_EXACT_OUT);
    }

    /// `RouteArgsV2` round-trips with multiple hops and mixed leg kinds.
    #[test]
    fn route_args_v2_borsh_roundtrip() {
        let args = RouteArgsV2 {
            amount_in: 5_000_000,
            min_return: 4_900_000,
            hops: vec![
                HopMetaV2 {
                    program_id_index: 0,
                    source_token_index: 1,
                    destination_token_index: 2,
                    amount_in_offset: 8,
                    splice_xor_mask: [0; 8],
                    source_kind: LEG_TOKEN,
                    dest_kind: LEG_TOKEN,
                    accounts: vec![HopAccountMeta::new(1, true, false)],
                    data: vec![0u8; 24],
                },
                HopMetaV2 {
                    program_id_index: 4,
                    source_token_index: 5,
                    destination_token_index: 6,
                    amount_in_offset: 8,
                    splice_xor_mask: [0xa5; 8],
                    source_kind: LEG_NATIVE,
                    dest_kind: LEG_TOKEN,
                    accounts: vec![HopAccountMeta::new(4, true, true)],
                    data: vec![0xff; 25],
                },
            ],
            fee_bps: 300,
            fee_on_input: false,
            sa_native_topup: 0,
        };
        let bytes = args.try_to_vec().unwrap();
        let back = RouteArgsV2::try_from_slice(&bytes).unwrap();
        assert_eq!(args, back);
    }

    /// Envelope = 31 B for both legacy args; hop payload = `23 + 2·A + D`.
    /// Mirror these two numbers in any hand-written client.
    #[test]
    fn wire_envelope_and_hop_payload_are_pinned() {
        let v2 = RouteArgsV2 {
            amount_in: 1,
            min_return: 2,
            hops: vec![],
            fee_bps: 3,
            fee_on_input: true,
            sa_native_topup: 4,
        };
        let eo = RouteArgsExactOut {
            amount_out: 1,
            max_amount_in: 2,
            hops: vec![],
            fee_bps: 3,
            fee_on_input: true,
            sa_native_topup: 4,
        };
        let v2_len = v2.try_to_vec().unwrap().len();
        let eo_len = eo.try_to_vec().unwrap().len();
        assert_eq!(v2_len, 31, "RouteArgsV2 envelope drifted from the wire model");
        assert_eq!(eo_len, 31, "RouteArgsExactOut envelope drifted from the wire model");
        assert_eq!(v2_len, eo_len, "the two ix must share one envelope size");

        for (a, d) in [(0usize, 0usize), (1, 16), (7, 40)] {
            let hop = HopMetaV2 {
                program_id_index: 0,
                source_token_index: 1,
                destination_token_index: 2,
                amount_in_offset: 8,
                splice_xor_mask: [0u8; 8],
                source_kind: LEG_TOKEN,
                dest_kind: LEG_TOKEN,
                accounts: (0..a).map(|i| HopAccountMeta::new(i as u8, true, false)).collect(),
                data: vec![0xAB; d],
            };
            let with_hop = RouteArgsV2 {
                hops: vec![hop],
                ..v2.clone()
            };
            assert_eq!(
                with_hop.try_to_vec().unwrap().len(),
                31 + 23 + 2 * a + d,
                "hop payload drifted for (accounts={a}, data={d})"
            );
        }
    }
}
