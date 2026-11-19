//! Country identifier validation for `KovaraIndex`.
//!
//! Every write and read entry point rejects unsupported country codes before
//! any authorization check or storage mutation, matching the guard
//! `PriceVault` applies via its `registry::is_supported_country` predicate.
//!
//! # What is tested
//!
//! * **Supported codes** — a representative sample of the 20 registry entries
//!   are accepted by all three write paths.
//! * **Unsupported codes** — garbage strings, a plausible-but-unlisted code,
//!   lowercase variants, and an empty-ish symbol all produce
//!   `Error::InvalidCountry` from every write path, and `None` from every
//!   read path, without touching storage.
//! * **Storage is unchanged on failure** — after every rejected write the
//!   previously stored value (or absence of one) is verified to be intact.

use kovara_index::{Error, KovaraIndex, KovaraIndexClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn setup(env: &Env) -> (KovaraIndexClient<'_>, Address) {
    env.mock_all_auths();
    let contract_id = env.register(KovaraIndex, ());
    (KovaraIndexClient::new(env, &contract_id), Address::generate(env))
}

/// A window and query_ts guaranteed to pass all freshness checks.
const VF: u64 = 1_000;
const VU: u64 = 2_000;
const QT: u64 = 1_500;

// ── Supported codes are accepted ─────────────────────────────────────────────

#[test]
fn set_daily_index_accepts_every_registry_entry() {
    let env = Env::default();
    let (client, updater) = setup(&env);

    // Use a different date for each country so write-once never fires.
    let supported = [
        "US", "GB", "NG", "KE", "IN", "BR", "DE", "FR", "JP", "CN",
        "ZA", "GH", "EG", "TZ", "UG", "ET", "PH", "ID", "MX", "AR",
    ];
    for (date, code) in supported.iter().enumerate() {
        let country = Symbol::new(&env, code);
        assert_eq!(
            client.try_set_daily_index(&updater, &country, &(date as u64 + 1), &10_000),
            Ok(Ok(())),
            "set_daily_index should accept supported country {code}"
        );
    }
}

#[test]
fn set_daily_index_windowed_accepts_every_registry_entry() {
    let env = Env::default();
    let (client, updater) = setup(&env);

    let supported = [
        "US", "GB", "NG", "KE", "IN", "BR", "DE", "FR", "JP", "CN",
        "ZA", "GH", "EG", "TZ", "UG", "ET", "PH", "ID", "MX", "AR",
    ];
    for (date, code) in supported.iter().enumerate() {
        let country = Symbol::new(&env, code);
        assert_eq!(
            client.try_set_daily_index_windowed(
                &updater,
                &country,
                &(date as u64 + 1),
                &10_000,
                &VF,
                &VU,
                &QT,
            ),
            Ok(Ok(())),
            "set_daily_index_windowed should accept supported country {code}"
        );
    }
}

#[test]
fn correct_daily_index_accepts_a_supported_country() {
    let env = Env::default();
    let (client, updater) = setup(&env);
    let authority = Address::generate(&env);
    let country = Symbol::new(&env, "ZA");

    client.set_correction_authority(&authority);
    client.set_daily_index(&updater, &country, &1, &10_000);

    assert_eq!(
        client.try_correct_daily_index(&authority, &country, &1, &10_050),
        Ok(Ok(()))
    );
    assert_eq!(client.get_daily_index(&country, &1), Some(10_050));
}

// ── Unsupported codes are rejected on write paths ────────────────────────────

/// Codes that must never pass: not in the 20-entry registry.
fn unsupported_codes(env: &Env) -> [Symbol; 6] {
    [
        Symbol::new(env, "XX"),   // plausible-looking but unlisted
        Symbol::new(env, "ZZ"),   // ISO 3166 user-assigned / unlisted
        Symbol::new(env, "ng"),   // lowercase variant of a valid code
        Symbol::new(env, "us"),   // lowercase variant of a valid code
        Symbol::new(env, "USA"),  // alpha-3 instead of alpha-2
        Symbol::new(env, "FOOD"), // not a country code at all
    ]
}

#[test]
fn set_daily_index_rejects_unsupported_country_before_any_write() {
    let env = Env::default();
    let (client, updater) = setup(&env);

    for (date, country) in unsupported_codes(&env).iter().enumerate() {
        let date = date as u64 + 1;
        assert_eq!(
            client.try_set_daily_index(&updater, country, &date, &10_000),
            Err(Ok(Error::InvalidCountry)),
            "set_daily_index should reject unsupported country"
        );
        // Storage must be untouched.
        assert_eq!(
            client.get_daily_index(country, &date),
            None,
            "storage must be unchanged after rejected set_daily_index"
        );
    }
}

#[test]
fn set_daily_index_windowed_rejects_unsupported_country_before_any_write() {
    let env = Env::default();
    let (client, updater) = setup(&env);

    for (date, country) in unsupported_codes(&env).iter().enumerate() {
        let date = date as u64 + 1;
        assert_eq!(
            client.try_set_daily_index_windowed(
                &updater, country, &date, &10_000, &VF, &VU, &QT,
            ),
            Err(Ok(Error::InvalidCountry)),
            "set_daily_index_windowed should reject unsupported country"
        );
        // Neither the value nor the window may have been written.
        assert_eq!(
            client.get_daily_index(country, &date),
            None,
            "value must be absent after rejected windowed write"
        );
        assert_eq!(
            client.get_daily_window(country, &date),
            None,
            "window must be absent after rejected windowed write"
        );
    }
}

#[test]
fn correct_daily_index_rejects_unsupported_country_before_any_write() {
    let env = Env::default();
    let (client, updater) = setup(&env);
    let authority = Address::generate(&env);

    // Set up authority and a valid entry for a supported country.
    let supported = Symbol::new(&env, "NG");
    client.set_correction_authority(&authority);
    client.set_daily_index(&updater, &supported, &1, &10_000);

    for (date, country) in unsupported_codes(&env).iter().enumerate() {
        let date = date as u64 + 1;
        assert_eq!(
            client.try_correct_daily_index(&authority, country, &date, &99_999),
            Err(Ok(Error::InvalidCountry)),
            "correct_daily_index should reject unsupported country"
        );
    }

    // The legitimate entry must be undisturbed.
    assert_eq!(client.get_daily_index(&supported, &1), Some(10_000));
}

// ── Unsupported codes on read paths return None, not a panic ─────────────────

#[test]
fn get_daily_index_returns_none_for_unsupported_country() {
    let env = Env::default();
    let (client, _) = setup(&env);

    for country in unsupported_codes(&env).iter() {
        assert_eq!(
            client.get_daily_index(country, &1),
            None,
            "get_daily_index must return None for unsupported country"
        );
    }
}

#[test]
fn get_daily_window_returns_none_for_unsupported_country() {
    let env = Env::default();
    let (client, _) = setup(&env);

    for country in unsupported_codes(&env).iter() {
        assert_eq!(
            client.get_daily_window(country, &1),
            None,
            "get_daily_window must return None for unsupported country"
        );
    }
}

#[test]
fn get_fresh_daily_index_returns_none_for_unsupported_country() {
    let env = Env::default();
    let (client, _) = setup(&env);

    for country in unsupported_codes(&env).iter() {
        assert_eq!(
            client.get_fresh_daily_index(country, &1, &QT),
            None,
            "get_fresh_daily_index must return None for unsupported country"
        );
    }
}

// ── InvalidCountry fires before other argument checks ────────────────────────
//
// The check order documented in the entry-point doc comments is:
//   InvalidCountry → InvalidDate → InvalidWindow → … → DuplicateWrite
//
// Supplying an unsupported country *and* a zero date must still produce
// InvalidCountry, not InvalidDate, so callers can rely on the documented order.

#[test]
fn invalid_country_takes_precedence_over_invalid_date() {
    let env = Env::default();
    let (client, updater) = setup(&env);
    let bad = Symbol::new(&env, "XX");

    // date == 0 would normally fire InvalidDate, but country is checked first.
    assert_eq!(
        client.try_set_daily_index(&updater, &bad, &0, &10_000),
        Err(Ok(Error::InvalidCountry))
    );
    assert_eq!(
        client.try_set_daily_index_windowed(&updater, &bad, &0, &10_000, &VF, &VU, &QT),
        Err(Ok(Error::InvalidCountry))
    );
}

#[test]
fn invalid_country_takes_precedence_over_invalid_window() {
    let env = Env::default();
    let (client, updater) = setup(&env);
    let bad = Symbol::new(&env, "ZZ");

    // valid_until == valid_from would normally fire InvalidWindow.
    assert_eq!(
        client.try_set_daily_index_windowed(&updater, &bad, &1, &10_000, &VF, &VF, &QT),
        Err(Ok(Error::InvalidCountry))
    );
}
