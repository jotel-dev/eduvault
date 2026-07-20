#![cfg(test)]

extern crate std;

use crate::{
    AssetKind, AssetQuote, MaterialRecord, MaterialStatus, PayoutShare, PurchaseManager,
    PurchaseManagerClient,
};
use proptest::prelude::*;
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Ledger},
    vec, Address, Bytes, BytesN, Env, IntoVal, Symbol, Vec,
};
use std::collections::BTreeMap;

#[contracttype]
#[derive(Clone)]
enum MockRegistryKey {
    Material(BytesN<32>),
}

#[contract]
struct MockRegistry;

#[contractimpl]
impl MockRegistry {
    pub fn set_material(env: Env, material_id: BytesN<32>, material: MaterialRecord) {
        env.storage()
            .persistent()
            .set(&MockRegistryKey::Material(material_id), &material);
    }

    pub fn get_material(
        env: Env,
        material_id: BytesN<32>,
    ) -> Result<MaterialRecord, crate::PurchaseError> {
        env.storage()
            .persistent()
            .get(&MockRegistryKey::Material(material_id))
            .ok_or(crate::PurchaseError::MaterialNotFound)
    }
}

// Minimal SAC mock using token::Client or simply custom as in test.rs
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct MockTransfer {
    from: Address,
    to: Address,
    amount: i128,
}

#[contracttype]
#[derive(Clone)]
enum MockAssetKey {
    Transfers,
}

#[contract]
struct MockAsset;

#[contractimpl]
impl MockAsset {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let mut transfers: Vec<MockTransfer> = env
            .storage()
            .persistent()
            .get(&MockAssetKey::Transfers)
            .unwrap_or(vec![&env]);
        transfers.push_back(MockTransfer { from, to, amount });
        env.storage()
            .persistent()
            .set(&MockAssetKey::Transfers, &transfers);
    }
    pub fn balance(_env: Env, _id: Address) -> i128 {
        1_000_000_000 // Always enough for tests
    }
}

const NUM_ACTORS: u8 = 4;
const NUM_MATERIALS: u8 = 3;
const NUM_ASSETS: u8 = 2;

#[derive(Clone, Debug)]
enum Command {
    SetAssetAllowed {
        asset_idx: u8,
        enabled: bool,
    },
    UpdateMaterial {
        mat_idx: u8,
        creator_idx: u8,
        asset_idx: u8,
        price: i128,
        status: MaterialStatus,
        paused: bool,
    },
    Purchase {
        buyer_idx: u8,
        mat_idx: u8,
        asset_idx: u8,
        expected_amount: i128,
    },
    WithdrawPayout {
        actor_idx: u8,
        purchase_id: u64,
        advance_ledgers: u32,
    },
}

fn gen_status() -> impl Strategy<Value = MaterialStatus> {
    prop_oneof![
        Just(MaterialStatus::Active),
        Just(MaterialStatus::Paused),
        Just(MaterialStatus::Archived),
    ]
}

fn gen_command() -> impl Strategy<Value = Command> {
    prop_oneof![
        (0..NUM_ASSETS, any::<bool>())
            .prop_map(|(asset_idx, enabled)| Command::SetAssetAllowed { asset_idx, enabled }),
        (
            0..NUM_MATERIALS,
            0..NUM_ACTORS,
            0..NUM_ASSETS,
            1..100_000i128,
            gen_status(),
            any::<bool>()
        )
            .prop_map(|(mat_idx, creator_idx, asset_idx, price, status, paused)| {
                Command::UpdateMaterial {
                    mat_idx,
                    creator_idx,
                    asset_idx,
                    price,
                    status,
                    paused,
                }
            }),
        (
            0..NUM_ACTORS,
            0..NUM_MATERIALS,
            0..NUM_ASSETS,
            0..100_000i128
        )
            .prop_map(|(buyer_idx, mat_idx, asset_idx, expected_amount)| {
                Command::Purchase {
                    buyer_idx,
                    mat_idx,
                    asset_idx,
                    expected_amount,
                }
            }),
        (0..NUM_ACTORS, 0..10u64, 0..50_000u32).prop_map(
            |(actor_idx, purchase_id, advance_ledgers)| Command::WithdrawPayout {
                actor_idx,
                purchase_id,
                advance_ledgers
            }
        ),
    ]
}

#[derive(Clone, Debug, Default)]
struct Model {
    assets: BTreeMap<u8, bool>,
    materials: BTreeMap<u8, MaterialRecord>,
    purchases: BTreeMap<u64, (u8, u8)>, // purchase_id -> (buyer_idx, mat_idx)
    claimed: BTreeMap<u64, bool>,
    purchase_nonce: u64,
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(if std::env::var("PROPTEST_CASES").is_ok() { std::env::var("PROPTEST_CASES").unwrap().parse().unwrap_or(256) } else { 256 }))]
    #[test]
    fn fuzz_purchase_manager(commands in proptest::collection::vec(gen_command(), 1..20)) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(PurchaseManager, ());
        let client = PurchaseManagerClient::new(&env, &contract_id);

        let mock_registry_id = env.register(MockRegistry, ());

        let mut actors = std::vec::Vec::new();
        for _ in 0..NUM_ACTORS {
            actors.push(Address::generate(&env));
        }
        let admin = actors[0].clone();
        let treasury = actors[1].clone();

        let mut assets = std::vec::Vec::new();
        for _ in 0..NUM_ASSETS {
            assets.push(env.register(MockAsset, ()));
        }

        let mut materials = std::vec::Vec::new();
        for i in 0..NUM_MATERIALS {
            materials.push(BytesN::from_array(&env, &[i; 32]));
        }

        client.initialize(&admin, &mock_registry_id, &treasury, &500);

        let mut model = Model::default();

        for cmd in commands {
            match cmd {
                Command::SetAssetAllowed { asset_idx, enabled } => {
                    let asset = &assets[asset_idx as usize];
                    client.set_asset_allowed(&admin, asset, &AssetKind::Token, &enabled);
                    model.assets.insert(asset_idx, enabled);
                }
                Command::UpdateMaterial { mat_idx, creator_idx, asset_idx, price, status, paused } => {
                    let mat_id = &materials[mat_idx as usize];
                    let creator = &actors[creator_idx as usize];
                    let asset = &assets[asset_idx as usize];

                    let mut quotes = Vec::new(&env);
                    quotes.push_back(AssetQuote { asset: asset.clone(), amount: price });

                    let mut payout_shares = Vec::new(&env);
                    payout_shares.push_back(PayoutShare { recipient: creator.clone(), share_bps: 10_000 });

                    let record = MaterialRecord {
                        material_id: mat_id.clone(),
                        creator: creator.clone(),
                        paused,
                        status,
                        quotes,
                        payout_shares,
                    };

                    env.invoke_contract::<()>(
                        &mock_registry_id,
                        &Symbol::new(&env, "set_material"),
                        vec![&env, mat_id.into_val(&env), record.into_val(&env)],
                    );

                    model.materials.insert(mat_idx, record);
                }
                Command::Purchase { buyer_idx, mat_idx, asset_idx, expected_amount } => {
                    let buyer = &actors[buyer_idx as usize];
                    let mat_id = &materials[mat_idx as usize];
                    let asset = &assets[asset_idx as usize];
                    let tx_id = Bytes::from_array(&env, b"tx123");

                    let res = client.try_purchase(buyer, mat_id, asset, &expected_amount, &tx_id);

                    let mut expected_ok = false;

                    if let Some(mat) = model.materials.get(&mat_idx) {
                        if !mat.paused && mat.status == MaterialStatus::Active {
                            if *model.assets.get(&asset_idx).unwrap_or(&false) {
                                // check if buyer already has it
                                let mut has_entitlement = false;
                                for (_, (b_idx, m_idx)) in model.purchases.iter() {
                                    if *b_idx == buyer_idx && *m_idx == mat_idx {
                                        has_entitlement = true;
                                        break;
                                    }
                                }

                                if !has_entitlement {
                                    if mat.quotes.get_unchecked(0).amount == expected_amount {
                                        expected_ok = true;
                                    }
                                }
                            }
                        }
                    }

                    assert_eq!(res.is_ok(), expected_ok, "Purchase mismatch. Res: {:?}", res);

                    if expected_ok {
                        model.purchases.insert(model.purchase_nonce, (buyer_idx, mat_idx));
                        model.purchase_nonce += 1;
                        assert!(client.has_entitlement(mat_id, buyer));
                    }
                }
                Command::WithdrawPayout { actor_idx, purchase_id, advance_ledgers } => {
                    let actor = &actors[actor_idx as usize];
                    let current_ledger = env.ledger().sequence();
                    env.ledger().set_sequence_number(current_ledger + advance_ledgers);

                    let txn_id = Bytes::from_array(&env, b"12345678901234567890123456789012");
                    let res = client.try_withdraw_payouts(actor, &purchase_id, &txn_id);

                    let mut expected_ok = false;

                    if model.purchases.contains_key(&purchase_id) {
                        if !model.claimed.get(&purchase_id).unwrap_or(&false) {
                            if let Some(escrow) = client.get_escrow_record(&purchase_id) {
                                let mut is_recipient = false;
                                for share in escrow.payout_shares.iter() {
                                    if share.recipient == *actor {
                                        is_recipient = true;
                                        break;
                                    }
                                }

                                if is_recipient && env.ledger().sequence() >= escrow.purchase_ledger + 35_000 {
                                    expected_ok = true;
                                }
                            }
                        }
                    }

                    assert_eq!(res.is_ok(), expected_ok, "Withdraw mismatch. Res: {:?}", res);

                    if expected_ok {
                        model.claimed.insert(purchase_id, true);
                    }
                }
            }
        }
    }
}

#[test]
#[should_panic]
fn test_mutant_violation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PurchaseManager, ());
    let client = PurchaseManagerClient::new(&env, &contract_id);

    let mock_registry_id = env.register(MockRegistry, ());
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Initializing twice should fail (invariant). If we try to init twice and assert it succeeds, it simulates a mutant where the check is missing.
    client.initialize(&admin, &mock_registry_id, &treasury, &500);
    let res = client.try_initialize(&admin, &mock_registry_id, &treasury, &500);
    assert!(
        res.is_ok(),
        "Expected success but got error! (Mutant caught)"
    );
}
