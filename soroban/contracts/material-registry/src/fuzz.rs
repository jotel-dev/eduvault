#![cfg(test)]

extern crate std;

use crate::{
    AssetKind, AssetQuote, MaterialRegistry, MaterialRegistryClient, MaterialStatus, PayoutShare,
};
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, BytesN, Env, String, Vec,
};
use std::collections::BTreeMap;

const NUM_ACTORS: u8 = 5;
const NUM_ASSETS: u8 = 3;

#[derive(Clone, Debug)]
enum Command {
    SetAssetAllowed {
        actor_idx: u8,
        asset_idx: u8,
        kind: AssetKind,
        enabled: bool,
    },
    RegisterMaterial {
        creator_idx: u8,
        asset_idx: u8,
        amount: i128,
        share_bps: u32,
    },
    SetMaterialStatus {
        actor_idx: u8,
        mat_idx: usize,
        status: MaterialStatus,
    },
}

fn gen_asset_kind() -> impl Strategy<Value = AssetKind> {
    prop_oneof![
        Just(AssetKind::Native),
        Just(AssetKind::Token),
        Just(AssetKind::CreatorToken),
    ]
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
        (0..NUM_ACTORS, 0..NUM_ASSETS, gen_asset_kind(), any::<bool>()).prop_map(
            |(actor_idx, asset_idx, kind, enabled)| Command::SetAssetAllowed {
                actor_idx,
                asset_idx,
                kind,
                enabled
            }
        ),
        (0..NUM_ACTORS, 0..NUM_ASSETS, 1..i128::MAX).prop_map(
            |(creator_idx, asset_idx, amount)| Command::RegisterMaterial {
                creator_idx,
                asset_idx,
                amount,
                share_bps: 10_000,
            }
        ),
        (0..NUM_ACTORS, any::<usize>(), gen_status()).prop_map(
            |(actor_idx, mat_idx, status)| Command::SetMaterialStatus {
                actor_idx,
                mat_idx,
                status
            }
        ),
    ]
}

#[derive(Clone, Debug)]
struct MaterialState {
    creator: u8,
    status: MaterialStatus,
    paused: bool,
}

#[derive(Clone, Debug, Default)]
struct Model {
    admin: Option<u8>,
    assets: BTreeMap<u8, bool>,
    materials: std::vec::Vec<MaterialState>,
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(if std::env::var("PROPTEST_CASES").is_ok() { std::env::var("PROPTEST_CASES").unwrap().parse().unwrap_or(256) } else { 256 }))]
    #[test]
    fn fuzz_material_registry(commands in proptest::collection::vec(gen_command(), 1..20)) {
        let env = Env::default();
        env.mock_all_auths();
        
        let contract_id = env.register_contract(None, MaterialRegistry);
        let client = MaterialRegistryClient::new(&env, &contract_id);
        
        let mut actors = std::vec::Vec::new();
        for _ in 0..NUM_ACTORS {
            actors.push(Address::generate(&env));
        }
        
        let mut assets = std::vec::Vec::new();
        for _ in 0..NUM_ASSETS {
            assets.push(Address::generate(&env));
        }

        let mut model = Model::default();
        let mut material_ids: std::vec::Vec<BytesN<32>> = std::vec::Vec::new();

        for cmd in commands {
            match cmd {
                Command::SetAssetAllowed { actor_idx, asset_idx, kind, enabled } => {
                    let admin_addr = &actors[actor_idx as usize];
                    let asset_addr = &assets[asset_idx as usize];
                    
                    let res = client.try_set_asset_allowed(admin_addr, asset_addr, &kind, &enabled);
                    
                    let mut is_ok = false;
                    if let Some(admin) = model.admin {
                        if admin == actor_idx {
                            is_ok = true;
                            model.assets.insert(asset_idx, enabled);
                        }
                    }
                    
                    assert_eq!(res.is_ok(), is_ok, "SetAssetAllowed mismatch");
                }
                Command::RegisterMaterial { creator_idx, asset_idx, amount, share_bps } => {
                    let creator = &actors[creator_idx as usize];
                    let asset = &assets[asset_idx as usize];
                    
                    let mut quotes = Vec::new(&env);
                    quotes.push_back(AssetQuote { asset: asset.clone(), amount });
                    
                    let mut shares = Vec::new(&env);
                    shares.push_back(PayoutShare { recipient: creator.clone(), share_bps });

                    let metadata_uri = String::from_str(&env, "ipfs://test");
                    let metadata_hash = BytesN::from_array(&env, &[0; 32]);
                    let rights_hash = BytesN::from_array(&env, &[0; 32]);
                    
                    let res = client.try_register_material(
                        creator, &metadata_uri, &metadata_hash, &rights_hash, &quotes, &shares
                    );

                    let mut expected_success = false;
                    
                    if model.admin.is_none() {
                        expected_success = true;
                        model.admin = Some(creator_idx);
                    } else {
                        if *model.assets.get(&asset_idx).unwrap_or(&false) {
                            expected_success = true;
                        }
                    }

                    assert_eq!(res.is_ok(), expected_success, "RegisterMaterial mismatch.");

                    if res.is_ok() {
                        let mat_id = res.unwrap().unwrap();
                        material_ids.push(mat_id);
                        model.materials.push(MaterialState {
                            creator: creator_idx,
                            status: MaterialStatus::Active,
                            paused: false,
                        });
                    }
                }
                Command::SetMaterialStatus { actor_idx, mat_idx, status } => {
                    if material_ids.is_empty() {
                        continue;
                    }
                    let m_idx = mat_idx % material_ids.len();
                    let mat_id = &material_ids[m_idx];
                    let actor = &actors[actor_idx as usize];
                    let state = &mut model.materials[m_idx];

                    let res = client.try_set_material_status(actor, mat_id, &status);

                    let mut expected_success = false;
                    let is_admin = model.admin == Some(actor_idx);
                    if state.creator == actor_idx || is_admin {
                        expected_success = true;
                        state.status = status;
                        state.paused = status == MaterialStatus::Paused;
                    }

                    assert_eq!(res.is_ok(), expected_success, "SetMaterialStatus mismatch");
                    
                    if res.is_ok() {
                        let on_chain = client.get_material(mat_id);
                        assert_eq!(on_chain.status, state.status);
                        assert_eq!(on_chain.paused, state.paused);
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
    
    let contract_id = env.register_contract(None, MaterialRegistry);
    let client = MaterialRegistryClient::new(&env, &contract_id);
    
    let creator = Address::generate(&env);
    let asset = Address::generate(&env);
    
    let mut quotes = Vec::new(&env);
    quotes.push_back(AssetQuote { asset: asset.clone(), amount: 0 }); 
    
    let mut shares = Vec::new(&env);
    shares.push_back(PayoutShare { recipient: creator.clone(), share_bps: 10_000 });
    
    let metadata_uri = String::from_str(&env, "ipfs://test");
    let metadata_hash = BytesN::from_array(&env, &[0; 32]);
    let rights_hash = BytesN::from_array(&env, &[0; 32]);
    
    let res = client.try_register_material(
        &creator, &metadata_uri, &metadata_hash, &rights_hash, &quotes, &shares
    );
    
    assert!(res.is_ok(), "Expected success but got error! (Mutant caught)");
}
