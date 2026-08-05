use anchor_lang::prelude::*;

use crate::error::ErrorCode;

/// Seed for the swap-authority (SA) PDA that holds funds mid-route.
#[constant]
pub const SEED_SA: &[u8] = b"raze-router";

/// Number of shared swap-authority PDAs wallets are hashed across. A power of
/// two, so the bucket is a mask; the reusable SA accounts stay a fixed set that
/// fits in a lookup table.
pub const SA_POOL_SIZE: u8 = 64;

/// Pool bucket for a payer: XOR-fold of the pubkey bytes, masked to the size.
/// Whatever builds a route must derive the same bucket.
pub fn sa_pool_index(payer: &Pubkey) -> u8 {
    payer.to_bytes().iter().fold(0u8, |acc, b| acc ^ b) & (SA_POOL_SIZE - 1)
}

/// Match the passed authority against `SA_POOL[sa_pool_index(payer)]` and return
/// `(bump, index)` for the `invoke_signed` seeds.
pub fn resolve_sa(sa_key: &Pubkey, payer: &Pubkey) -> Result<(u8, u8)> {
    let index = sa_pool_index(payer);
    let (pooled, bump) = SA_POOL[index as usize];
    if *sa_key == pooled {
        return Ok((bump, index));
    }
    err!(ErrorCode::InvalidSwapAuthority)
}

/// The singleton `RouterConfig` PDA (`[SEED_CONFIG]`, bump 254).
pub const CONFIG_ADDRESS: Pubkey =
    Pubkey::from_str_const("9nKMpsQiZfDjMRWzGpBuCZfge2Ra2uZLAav4JHXCx1iE");

/// The pooled swap-authority PDAs `(address, bump)` from `[SEED_SA, [i]]`.
pub const SA_POOL: [(Pubkey, u8); SA_POOL_SIZE as usize] = [
    (Pubkey::from_str_const("FgQDhc1poWcp8Cm4TbgJtZZqqybUU5zhmpdjqzhCCvjA"), 254),
    (Pubkey::from_str_const("2eArhZJj2ziaXQzDhLyAUi96VhGu4xVB5RYUEtKB5rYo"), 255),
    (Pubkey::from_str_const("A3SK2JxCRCETKcmKLqc1y2Vq1r4P2v69FYsWwuvrbuoH"), 254),
    (Pubkey::from_str_const("J8A8AGfiz1JH3yWo3P5izYCdWyWSFWPoiWszxwUfXZ28"), 255),
    (Pubkey::from_str_const("gWLNijM9mmDdzXjnwHeQawjgPaB35bLphASXP3yNKyc"), 255),
    (Pubkey::from_str_const("CfMj6nPcVGr27y5u7KQkpRf16V3YS8Q5whaDereJP38X"), 253),
    (Pubkey::from_str_const("ANfFc6vDoRqhUNVyry3NhqWw7ReYuKsgYqoX5ZGsYbKe"), 253),
    (Pubkey::from_str_const("5unLPYLK4wRex71jn9BFkdhdFw3Q2yJCkpEYVjkPgdEA"), 255),
    (Pubkey::from_str_const("J7MWn2WjAVU5mNCPBa9X1VDxQKYUodA4NLME4qbs144d"), 252),
    (Pubkey::from_str_const("35WWG6gf68TCC1fcL1uwECf7mDD6q4RFbeQEiPBhCgvL"), 255),
    (Pubkey::from_str_const("7zXxrHEkrte2EBXT48TvYg65FTXVSw6Zrn12pkSDAAb1"), 254),
    (Pubkey::from_str_const("9U8AcPTky6rbzzPWwxRMjRPsmxwKeEHWaj91QPP6pQAZ"), 255),
    (Pubkey::from_str_const("9KiJh2pm8pNhoFazsRRx1M27aNTWjpXuv6HjChyQyzs2"), 255),
    (Pubkey::from_str_const("5VYa5pe3QiZcQo8jR21jTrmeR8hEWKZ33npnfxkH9BQ8"), 255),
    (Pubkey::from_str_const("81kjEbFAkU6N1seHPZwRgxGdtXstLuUoHzCTKkpmgrQn"), 255),
    (Pubkey::from_str_const("Hqzqa4UU6dzUQ4rXc2tTGVaFxtZ8mJdqaQZ2zgdFHjCm"), 255),
    (Pubkey::from_str_const("XFFnwyAUsioegazmsRWL5MWRW82dyEkySjAXnfKqABs"), 255),
    (Pubkey::from_str_const("2KRckUacbpDXg3gSSJ4of4DQchsz5GVPWBG9GgCq5GP9"), 255),
    (Pubkey::from_str_const("cVnm9pKMfrwprHx12o9eMsNvWfMdBNUqfyoBgEpDKsT"), 255),
    (Pubkey::from_str_const("Anz68sfAf6Bh8hBpt2Nx4L5uX5Dcd7CEsWAiSSBsx8si"), 255),
    (Pubkey::from_str_const("GFpA6SBpXUhHBq6oaKC16PutJS92MvapeQ3LCrCWPSR8"), 255),
    (Pubkey::from_str_const("EYmx5YDk6wL7wmPBbKagb8dcaCskg4HySnXgC6WGeTQX"), 253),
    (Pubkey::from_str_const("VUPigJ6yaTGkDjnSPnFeC1aoWofAZLPYrBJuaKwCARc"), 255),
    (Pubkey::from_str_const("772DAQn2uYFoqY2Y548DXj135ByC553jjB2ApPjJH4Cc"), 254),
    (Pubkey::from_str_const("A8K88FUjtUN3yAxASyQSEueyaepy2Gm5iwrWWZJCEt1X"), 255),
    (Pubkey::from_str_const("5LTnCG3z3kLgEoTWfkPiEpnkj9DZsgJ42KZZuRKQdV54"), 255),
    (Pubkey::from_str_const("AVXaVPVWceuo3tji6AXVCM6StNLxcfhm6JoEtLyfSrL5"), 255),
    (Pubkey::from_str_const("Gc7QZeEFvYMfZkgaDqmjjGsvzk3SNg1ubPrFeuboHvA2"), 255),
    (Pubkey::from_str_const("D4o1nmRaEa1fRp2rMC3P3cMJDC3rm1aekaynbqiVNtGU"), 252),
    (Pubkey::from_str_const("GiXvA25x6NpGSm1XwbMFV4wk3f8YEbLWtnGghJjvqZm3"), 255),
    (Pubkey::from_str_const("HVEZjqAXzGWEgu2TnAYEn7xeLAw6hBbExeVsunAPkL14"), 255),
    (Pubkey::from_str_const("4x5FUSv2cyyPbDApuEon881iuK5AP1hhXXmZBJdDtyVn"), 255),
    (Pubkey::from_str_const("8W6hJi1WG8Ym5oNscn1HHx7egCEvMBKzMCMcZ3bezbn5"), 255),
    (Pubkey::from_str_const("G8zNUoxThcV7V1Vpish8267APGY3EVgEJxcmQvhFQkhT"), 255),
    (Pubkey::from_str_const("FwYHDfHwa4KXEF1WpGeYYFJryaaYPpzNpPy97xiRNp32"), 255),
    (Pubkey::from_str_const("BpLqTgdtSVTVy1aFfaR8VmnYkjrundcYCGkDetKAWvK"), 255),
    (Pubkey::from_str_const("74rQ7z4fpiDTXscv2KwuJEuLQAhcf1xbStSyVWrwWRe6"), 253),
    (Pubkey::from_str_const("8MJKFESddePHxaGGpn4daXwRJ7BWKcyob92wb9JYQP32"), 255),
    (Pubkey::from_str_const("5AUXhTYp6Sv1yBd1K5KQb5LztzfwZRpabSj8yhx7xo7Z"), 254),
    (Pubkey::from_str_const("292pwLVxUwWWrTUXnDuR7qFQJyqATF793tTRCTLAcaax"), 249),
    (Pubkey::from_str_const("EdpjFYofrfg5DSXfTCGDpdqkG9cAtdgG4KpV81SsfNqQ"), 254),
    (Pubkey::from_str_const("G3uSUXRatMuQ2KKZCwf6MmhR3cemogfoiUTau5B6KPk"), 253),
    (Pubkey::from_str_const("5fYveBqar2Rvu1Sp5g6KAPFsTxJ44Ft8XLCgi5zmw4cq"), 253),
    (Pubkey::from_str_const("4CBGyK4Ed8T3UeZo59zmaacNhN8dVbwXb4QnJg8a6Y49"), 254),
    (Pubkey::from_str_const("JDCU7bhszAB7jKbxcRvVSwgCugPfCu4PeCKKzi9vGAFk"), 255),
    (Pubkey::from_str_const("BmpZiAGjHG5UFBGxTqpn4GH3tvnPE9U1GLaJ3ygqtC53"), 255),
    (Pubkey::from_str_const("6tENeTmwV1vo7WYj5xAoSXF1yAHa6Yxgireffzr5tBE8"), 254),
    (Pubkey::from_str_const("2Ba2bap8EkFG1sb1LB83BkUYcYJHBJ6utN9qvyVcM5aK"), 255),
    (Pubkey::from_str_const("42Ahyh1C34stdzJGVyhqB9PZYk57zb8GJKyUz1sAnkiH"), 255),
    (Pubkey::from_str_const("7XoiNdiuSKGsdMhqUBSbhwYxQda8YttKAtc9LLnoP9Ux"), 254),
    (Pubkey::from_str_const("7nji6koo3akv2raGpvGPSH2m5vbFf6XKEQS6CfScVFGW"), 255),
    (Pubkey::from_str_const("8GEbsUDRXvgSewa7YVFo54TQgbsQqpBNLNJ1ic8o3MUe"), 254),
    (Pubkey::from_str_const("Fg81TXbbXDnwKV9ieJ4KQTd3MGDFeWRU7E4h3VAhiTJS"), 255),
    (Pubkey::from_str_const("EuJDGHWkkEquhrp1awv2MmnrQ1pkCbMiKV6isK1Q525q"), 253),
    (Pubkey::from_str_const("8Bnhc7KpJGfQYHB4GqwPs5tS5dfP9ngeaakbZGBfC295"), 255),
    (Pubkey::from_str_const("DcgYxKqBendgSQ9w9GhsRvvsb6Hw88TdNuydCZRT6sto"), 255),
    (Pubkey::from_str_const("2PTgbE5sqHtxBRDduaRmp52fEGVNFF4QHusJuYvdxKQf"), 255),
    (Pubkey::from_str_const("GQ5XjtjaWZVjVFt9e7EKBUVZfp6Ng8Qsroh31J46EvLd"), 255),
    (Pubkey::from_str_const("9pzxWrRMjTHvxhANK2aVCJSxEDyFLARiMB9r4YZM33ST"), 254),
    (Pubkey::from_str_const("7BJ5FLhQ9Sj95w5xCwvVyTiDogKjK7AfxsqYRvmkHUyY"), 255),
    (Pubkey::from_str_const("7CrD7EFs8VCpUFy8HiN8RX8nQWjEev1xYF2aHuVckgRa"), 252),
    (Pubkey::from_str_const("64QU3AdtCuacisZ2daNMoycSeNjP8BL3MGgPo2oKVCPv"), 255),
    (Pubkey::from_str_const("8PJmzxpHLrhVK5Refq1oYVGgFPT2h742bHEcFjfPLzP5"), 254),
    (Pubkey::from_str_const("4XcrJXLMX9729kXe9nTxGUwnPX67hVEmdeHtxoifyPWn"), 255),
];

/// wSOL ATA of each pooled SA, same indexing as [`SA_POOL`].
pub const SA_WSOL_ATA: [Pubkey; SA_POOL_SIZE as usize] = [
    Pubkey::from_str_const("BU7LwRjK6N7ekYyVxFDARZiho8uJFxKW4qpaZ4bYZnHn"),
    Pubkey::from_str_const("6eAmaMbbuyxQmLf3r17F4q19SuZceR24MTYjPKA3AnDe"),
    Pubkey::from_str_const("HG6p5rm67PxtvL5c8LoLErFyKQMRu2qWG8Z83A65V9LA"),
    Pubkey::from_str_const("FgqMWD5KPcet3vRMHks9sdwKRMBApzaYcEHecrBo5B58"),
    Pubkey::from_str_const("EavAz98Ra3sf8GVTVoV47V2gwGVU3LvyuLdttpBAEWNt"),
    Pubkey::from_str_const("3B6RZLv3WrcaK5hK7SnCTuvv9ZvYpihJuA5BptVVXHza"),
    Pubkey::from_str_const("6x7sWGxmToVNEHGh4R77htwJtpmAfPE9cEcaMpwLJZW8"),
    Pubkey::from_str_const("8aczDntvk9e2apdeoq4T9PS8y4e8rMdXCCyBPHkU6WrS"),
    Pubkey::from_str_const("5qX1DoeTpXg1gdGKMRJso3uKLwhM1PB5hh6sYQMAZZTP"),
    Pubkey::from_str_const("6FRKjqdX3mrAKfaXcVXkQhdzQhizAYv4i4MrNcFcw2t4"),
    Pubkey::from_str_const("Hhtei4cZWMbZUNgxGo7Jv2N7bfEr8WKZvZfp9v7eZq4s"),
    Pubkey::from_str_const("DFTsK5XSXZfYyaFjGJ83pj96ZxLCTjRcXHSmRva1PXqF"),
    Pubkey::from_str_const("An9nfEbNHieGU8grExbE1Cap1GbWoccH5jqYTrZNteGw"),
    Pubkey::from_str_const("HCULDUJkN6KJZWUj116Evfej5HhpH6dQQ9D1ekezfJxJ"),
    Pubkey::from_str_const("HYqAcmaJR9pNgEp61jqRRqs8Mk9nfnHemu4bh8GBHtuX"),
    Pubkey::from_str_const("J69mTz6h9CCekHMYkjNBduPu2dE9dT3VtGpq3VTUPdcb"),
    Pubkey::from_str_const("J3TsEDg5ivq7TymP5u5DZpZLwLoN4BoWtMPS75eptzag"),
    Pubkey::from_str_const("ErHhNKyb5MzpMGGRirATVZiEWHeE5quuy1cvWWPQD8bW"),
    Pubkey::from_str_const("4thf1xMucpjLRtrQ6FnRDRpJBHSanDwjdTmwa9ZDo7jc"),
    Pubkey::from_str_const("Gnx7P1pG1Rkeq7wxcBpRFhaueUqGYKcsiRgPQMcxdxWr"),
    Pubkey::from_str_const("FB2wEvjajgCrBVhseFVvQCELwCvDQ2o7J1gZmviZezAj"),
    Pubkey::from_str_const("3haGUJF6mWyAZ3CtiedJQT4CSxbpMkkPbR1h6LWWihjZ"),
    Pubkey::from_str_const("GbDsAyJGWsvjZ1UeiB4k4QwZ3zWZKxkH5U91gziRVCHa"),
    Pubkey::from_str_const("6MqFEnpBLHUNJzP3p1kFFjL48qHbzRDUx3tDjUBorH3q"),
    Pubkey::from_str_const("9B1f1rQpzYSaH4gVfLEVzFaCLii7dX4HgQpeN7Rnyicy"),
    Pubkey::from_str_const("8dtcpQfdyrbuYEuVv2izA2soaS2ayaUfpXNqUx4Jrsub"),
    Pubkey::from_str_const("AADzZ7tkN1fMSi2NP6z4URL6Rp5f7iR4tHBsTeYrbaLY"),
    Pubkey::from_str_const("EhoeHhCsG2mhTv8B3nsf9RKg54oTzfZnHCPiYLqKDZzV"),
    Pubkey::from_str_const("G5Das6PtpHbFo9jyQmVsE8zwZPFKtFeMv5BmtTqur2SJ"),
    Pubkey::from_str_const("6h8tzppsBhtKH2VZ6agfZmPSEqKtna95fv6HcQx4HoaK"),
    Pubkey::from_str_const("DARKupTcqDt2j7HrD51PSYR8n2UsPa4AJx5zYNyLYBwa"),
    Pubkey::from_str_const("BTTTyEgsi414uYpR3CtMsdTw34Gioz3kkvwehfNYgqCH"),
    Pubkey::from_str_const("3i8UQFRckmYpC3QGw6Ejd2TtS2zS1simVyzDe5HecAXZ"),
    Pubkey::from_str_const("wUQBibJWMPGo8NyehkkqRtCAbiJHM6HhTkECKkUAufe"),
    Pubkey::from_str_const("Hxtzb5Utv5q9StdZ4i9z49Y5G1djxnkrTsh36Jombuqg"),
    Pubkey::from_str_const("95AnVTSABUo47k281t3f6seadWmQqWkjRLbyDJRp1rKA"),
    Pubkey::from_str_const("zpHYWFcQXu4tsz9rbzoeroVTrJE2iHa1NhZtRNtC8tB"),
    Pubkey::from_str_const("2i93sdA3GWpSKD8yfEEu1tX28yEoKHLVecVzWnSvgK1q"),
    Pubkey::from_str_const("Gsv8ojRm1HqPfpsfRJxLLHFamP2Htm9jJGp4rGx52d2f"),
    Pubkey::from_str_const("8TyS2xW5FCDMGU85EdjNJ5hLgqH6DSMj1G4ewj5SNgvu"),
    Pubkey::from_str_const("4bVaZorLm74bQVsLrQvaf3RV4jQQbkPjWPYbkvBnjxju"),
    Pubkey::from_str_const("AeABhRsw8DuMevmN1GWxXCzuGXuaSHi1vrKbtJGB9oFb"),
    Pubkey::from_str_const("J2Ta9rdA7JWPR8o6K8nnf7X6mrRXjuAqV7G6mcbkji73"),
    Pubkey::from_str_const("G9JdQPKrDTUYydgdCa4zUwaLCmLq1AoPupXkJDvtT6kF"),
    Pubkey::from_str_const("GhuCYK7eT3mqjbMqojCVxENBKcHFJAeEoVfuLRJyBVVU"),
    Pubkey::from_str_const("CWM4qkXZnNGnQFkz51t9Mh8iaa6YGgXgFV5haR8hVrGy"),
    Pubkey::from_str_const("BjwXaD4xMQT2k31NJw7pXbmvBiWssbstidnDafKQCd4Y"),
    Pubkey::from_str_const("GzheRLtRM56nuzUPo7sggp5uQFsZSYAF68kT5APUkUU3"),
    Pubkey::from_str_const("DrMfh87vfmtLd13BLbxcEhLMTKsRZPNryUTdL5xVSD73"),
    Pubkey::from_str_const("YettZDC6zCs1piix8RqWhVh9Wk8hhH5tabJoZjYrgLu"),
    Pubkey::from_str_const("GZHBsFP2NtNqe5bkJhGyrXJpSbTJJNgTrNQVTb21ij9K"),
    Pubkey::from_str_const("33Dagxk2aAHsM5NYq1cNPHFB2GGQfyjRegGwxMaH2N8w"),
    Pubkey::from_str_const("HfBF2cs5qUdQM5G8UYbzbjx5mD9YdmUvHttugTfCrfCw"),
    Pubkey::from_str_const("FmEoXynuJknX3ZtGStrhKaLJXsr8oVvugTMovppwd6Wx"),
    Pubkey::from_str_const("FfEa1ox8FQMzjpbjkrYq2cwnD1j6V1RUtppSA2fNfSbQ"),
    Pubkey::from_str_const("761NPxKXzTf7gXpg6xCXV7ByhUKs2jMEWo7PRVHpr5go"),
    Pubkey::from_str_const("3jAzLUhatZbxnyz8pznqYbjqwgTzwXFD5BMbcXXRWD3w"),
    Pubkey::from_str_const("A6cRRad7pocZ4Fa1amjTKFDi8W9cAf3KaWLDiDyU8Y67"),
    Pubkey::from_str_const("6QM8rQ3rquhmp2TP7Rp44t1YvkHFe8Wmy6V3dXtMYKfJ"),
    Pubkey::from_str_const("8y9HErRZHTJ5TmAyKagddR7f1TZiLzFRmLeUa4zx2AUb"),
    Pubkey::from_str_const("38BYTtE2PDMor741ZoL5CnircR2nNX8eoQ6vniDBcfqR"),
    Pubkey::from_str_const("BmQG8YB1hFnxAhGm9jgCWUuL2bzZ2oP7dSeqV8LFtYE8"),
    Pubkey::from_str_const("2NRbxZu5zLvVY4iSRHVGBaqAxTRV9FXzwPC66FWMfn1q"),
    Pubkey::from_str_const("GLarGvKht6ggU1pz3oW1AeKAj9F87v5oRYbpBEEtMiiN"),
];

/// Seed for the singleton `RouterConfig` PDA (admin, fee, venue allowlist).
#[constant]
pub const SEED_CONFIG: &[u8] = b"router-config";

/// Max hops in a single route.
pub const MAX_HOPS: usize = 3;

/// Max venue programs in the allowlist.
pub const MAX_VENUES: usize = 64;

/// Max CPI accounts a single hop may reference.
pub const MAX_ACCOUNTS_PER_HOP: usize = 32;

/// Cap (10%) on the per-route fee and the config fee.
pub const MAX_PLATFORM_FEE_BPS: u16 = 1_000;

/// Fee / bps denominator.
pub const FEE_BPS_DENOMINATOR: u64 = 10_000;

/// Minimum fraction of the fed amount a hop must consume.
pub const ACTUAL_IN_LOWER_BOUND_NUM: u128 = 90;
pub const ACTUAL_IN_LOWER_BOUND_DEN: u128 = 100;

/// SPL-token / Token-2022 base account layout offsets (shared by both).
pub const TOKEN_OWNER_OFFSET: usize = 32; // [32..64]
pub const TOKEN_AMOUNT_OFFSET: usize = 64; // [64..72]
pub const TOKEN_ACCOUNT_MIN_LEN: usize = 72;

/// SPL-token / Token-2022 base mint layout: `decimals: u8` sits after
/// mint_authority (4+32) + supply (8); extensions are appended after the base.
pub const MINT_DECIMALS_OFFSET: usize = 44; // [44]
pub const MINT_MIN_LEN: usize = 82;

pub const ZERO_ADDRESS: Pubkey = Pubkey::new_from_array([0u8; 32]);

/// Associated-token-account program id.
pub const ATA_PROGRAM: Pubkey =
    Pubkey::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

#[cfg(test)]
mod tests {
    use super::*;

    /// Shared vector: off-chain bucket derivation must match these three cases.
    #[test]
    fn sa_pool_index_shared_vector() {
        assert_eq!(SA_POOL_SIZE, 64);
        let mut b = [0u8; 32];
        b[0] = 0xFF;
        assert_eq!(sa_pool_index(&Pubkey::new_from_array(b)), 63);
        let mut b = [0u8; 32];
        b[0] = 0x01;
        b[1] = 0x02;
        assert_eq!(sa_pool_index(&Pubkey::new_from_array(b)), 3);
        let mut b = [0u8; 32];
        for (i, x) in b.iter_mut().enumerate() {
            *x = i as u8;
        }
        assert_eq!(sa_pool_index(&Pubkey::new_from_array(b)), 0);
    }

    /// Every const table entry must equal what `find_program_address` returns.
    #[test]
    fn pda_tables_match_derivation() {
        let ata_program: Pubkey =
            Pubkey::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
        let (config, config_bump) = Pubkey::find_program_address(&[SEED_CONFIG], &crate::ID);
        assert_eq!(config, CONFIG_ADDRESS);
        assert_eq!(config_bump, 254);
        for i in 0..SA_POOL_SIZE {
            let (sa, bump) = Pubkey::find_program_address(&[SEED_SA, &[i]], &crate::ID);
            assert_eq!((sa, bump), SA_POOL[i as usize], "SA_POOL[{i}]");
            let wsol_ata = Pubkey::find_program_address(
                &[
                    sa.as_ref(),
                    crate::shared::spl_token::ID.as_ref(),
                    crate::shared::wsol::ID.as_ref(),
                ],
                &ata_program,
            )
            .0;
            assert_eq!(wsol_ata, SA_WSOL_ATA[i as usize], "SA_WSOL_ATA[{i}]");
        }
    }

    /// Only the payer's own bucket resolves; another bucket or a per-signer
    /// derivation does not.
    #[test]
    fn resolve_sa_accepts_only_the_payers_pooled_bucket() {
        let payer = Pubkey::new_from_array([42u8; 32]);
        let index = sa_pool_index(&payer);
        let (sa, bump) = SA_POOL[index as usize];
        assert_eq!(resolve_sa(&sa, &payer).unwrap(), (bump, index));
        let other = SA_POOL[(index as usize + 1) % SA_POOL_SIZE as usize].0;
        assert!(resolve_sa(&other, &payer).is_err());
        let per_signer = Pubkey::find_program_address(&[SEED_SA, payer.as_ref()], &crate::ID).0;
        assert!(resolve_sa(&per_signer, &payer).is_err());
    }
}
