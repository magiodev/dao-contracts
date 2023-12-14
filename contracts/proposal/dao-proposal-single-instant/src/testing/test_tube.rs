#[cfg(test)]
pub mod test_tube {
    use crate::contract::derive_addr_from_pubkey;
    use crate::msg::{ExecuteMsg, InstantiateMsg, SingleChoiceInstantProposeMsg};
    use crate::state::VoteSignature;
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{to_binary, Api, Coin, Uint128};
    use cw_utils::Duration;
    use dao_interface::msg::InstantiateMsg as InstantiateMsgCore;
    use dao_interface::state::Admin;
    use dao_interface::state::ModuleInstantiateInfo;
    use dao_voting::pre_propose::PreProposeInfo;
    use dao_voting::threshold::Threshold;
    use dao_voting_cw4::msg::GroupContract;
    use osmosis_test_tube::Account;
    use osmosis_test_tube::{Module, OsmosisTestApp, SigningAccount, Wasm};
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Init constants
    const SLUG_DAO_DAO_CORE: &str = "dao_dao_core";
    const SLUG_CW4_GROUP: &str = "cw4_group";
    const SLUG_DAO_VOTING_CW4: &str = "dao_voting_cw4";
    const SLUG_DAO_PROPOSAL_SINGLE_INSTANT: &str = "dao_proposal_single_instant";

    /// Test constants
    const INITIAL_BALANCE_AMOUNT: u128 = 340282366920938463463374607431768211455u128;

    pub fn test_init(
        voters_number: u32,
    ) -> (
        OsmosisTestApp,
        HashMap<&'static str, String>,
        SigningAccount,
        Vec<SigningAccount>,
    ) {
        // Create new osmosis appchain instance
        let app = OsmosisTestApp::new();
        let wasm = Wasm::new(&app);

        // Create new admin account with initial funds
        // The contract admin, to be used during store code.
        let admin: SigningAccount = app
            .init_account(&[Coin::new(INITIAL_BALANCE_AMOUNT, "uosmo")])
            .unwrap();

        // Create voters accounts with initial funds
        let mut voters: Vec<SigningAccount> = vec![];
        for _ in 0..voters_number {
            voters.push(
                app.init_account(&[Coin::new(INITIAL_BALANCE_AMOUNT, "uosmo")])
                    .unwrap(),
            )
        }

        // Create a vector of cw4::Member
        let initial_members = voters
            .iter()
            .map(|voter| cw4::Member {
                addr: voter.address().to_string(),
                weight: 1,
            })
            .collect::<Vec<_>>();
        // TODO: Consider admin should be included ^ as member, but with voting power (weight) 0

        // Contracts to store and instantiate
        // TODO: Check testing::instantiate::instantiate_with_cw4_groups_governance()
        let contracts_setup: Vec<(&str, Vec<u8>)> = vec![
            (
                SLUG_CW4_GROUP,
                get_wasm_byte_code(SLUG_CW4_GROUP), // this is copy pasted from outside as this workspace if not creating this artifact. it has been taken from https://github.com/CosmWasm/cw-plus/tree/v1.1.0
            ),
            (SLUG_DAO_VOTING_CW4, get_wasm_byte_code(SLUG_DAO_VOTING_CW4)),
            (
                SLUG_DAO_PROPOSAL_SINGLE_INSTANT,
                get_wasm_byte_code(SLUG_DAO_PROPOSAL_SINGLE_INSTANT),
            ),
            (SLUG_DAO_DAO_CORE, get_wasm_byte_code(SLUG_DAO_DAO_CORE)),
        ];

        // Store contracts and declare a HashMap
        let code_ids: HashMap<&str, u64> = contracts_setup
            .iter()
            .map(|&(contract_name, ref wasm_byte_code)| {
                let code_id = wasm
                    .store_code(&wasm_byte_code, None, &admin)
                    .expect("Failed to store code")
                    .data
                    .code_id;

                (contract_name, code_id)
            })
            .collect();
        // TODO: Instantiate msgs defined here -> https://github.com/DA0-DA0/dao-contracts/wiki/Instantiating-a-DAO#proposal-module-instantiate-message

        // Instantiate contract and sub-contracts in once
        let dao_dao_core_instantiate_resp = wasm
            .instantiate(
                *code_ids.get(SLUG_DAO_DAO_CORE).unwrap(),
                &InstantiateMsgCore {
                    admin: Some(admin.address()),
                    name: "DAO DAO Core".to_string(),
                    description: "".to_string(),
                    image_url: None,
                    automatically_add_cw20s: true,
                    automatically_add_cw721s: true,
                    voting_module_instantiate_info: ModuleInstantiateInfo {
                        code_id: *code_ids.get(SLUG_DAO_VOTING_CW4).unwrap(),
                        msg: to_binary(&dao_voting_cw4::msg::InstantiateMsg {
                            group_contract: GroupContract::New {
                                cw4_group_code_id: *code_ids.get(SLUG_CW4_GROUP).unwrap(),
                                initial_members,
                            },
                        })
                        .unwrap(),
                        admin: Some(Admin::CoreModule {}),
                        funds: vec![],
                        label: "DAO DAO voting module".to_string(),
                    },
                    proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
                        code_id: *code_ids.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                        msg: to_binary(&InstantiateMsg {
                            threshold: Threshold::AbsoluteCount {
                                threshold: Uint128::new(1u128),
                            },
                            // TODO: Create an additional test variant as below
                            // threshold: Threshold::ThresholdQuorum {
                            //     threshold: PercentageThreshold,
                            //     quorum: PercentageThreshold,
                            // },
                            // max_voting_period: Duration::Time(0), // 0 seconds
                            max_voting_period: Duration::Height(1), // 0 blocks
                            min_voting_period: None,
                            only_members_execute: false, // TODO
                            allow_revoting: false,
                            pre_propose_info: PreProposeInfo::AnyoneMayPropose {}, // TODO
                            close_proposal_on_execution_failure: true,
                        })
                        .unwrap(),
                        admin: Some(Admin::CoreModule {}),
                        funds: vec![],
                        label: "DAO DAO governance module".to_string(),
                    }],
                    initial_items: None,
                    dao_uri: None,
                },
                Some(admin.address().as_str()),
                Some(SLUG_DAO_DAO_CORE),
                vec![].as_ref(),
                &admin,
            )
            .unwrap();

        // HashMap to store contract names and their addresses
        let mut contracts: HashMap<&str, String> = HashMap::new();

        for event in dao_dao_core_instantiate_resp.events {
            if event.ty == "wasm" {
                for attr in event.attributes {
                    match attr.key.as_str() {
                        "_contract_address" => {
                            contracts
                                .entry(SLUG_DAO_DAO_CORE)
                                .or_insert_with(|| attr.value.clone());
                        }
                        "voting_module" => {
                            contracts
                                .entry(SLUG_DAO_VOTING_CW4)
                                .or_insert_with(|| attr.value.clone());
                        }
                        "prop_module" => {
                            contracts
                                .entry(SLUG_DAO_PROPOSAL_SINGLE_INSTANT)
                                .or_insert_with(|| attr.value.clone());
                        }
                        _ => {}
                    }
                }
            }
        }

        // TODO: Assert that we have the required n. of contracts here, as the ^ nested for match could fail

        (app, contracts, admin, voters)
    }

    fn get_wasm_byte_code(filename: &str) -> Vec<u8> {
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let byte_code = std::fs::read(
            manifest_path
                .join("..")
                .join("..")
                .join("..")
                .join("artifacts")
                .join(format!("{}.wasm", filename)),
        );
        match byte_code {
            Ok(byte_code) => byte_code,
            // On arm processors, the above path is not found, so we try the following path
            Err(_) => std::fs::read(
                manifest_path
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("artifacts")
                    .join(format!("{}-aarch64.wasm", filename)),
            )
            .unwrap(),
        }
    }

    // Function to compute SHA256 hash of a message
    pub fn compute_sha256_hash(message: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(message);
        hasher.finalize().to_vec()
    }

    #[test]
    #[ignore]
    fn test_dao_proposal_single_instant() {
        let (app, contracts, admin, voters) = test_init(5);
        let wasm = Wasm::new(&app);

        // TODO: Mock signatures taking voter.publickey to recover the sig
        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for voter in voters {
            let clear_message = b"Hello World!";
            let message_hash = compute_sha256_hash(clear_message);
            let signature = voter.signing_key().sign(clear_message).unwrap();

            // VoteSignature
            vote_signatures.push(VoteSignature {
                message_hash,
                signature: signature.as_ref().to_vec(),
                public_key: voter.public_key().to_bytes(),
            })
        }

        // TODO: Do Admin send from admin to treasury
        // TODO: Get Admin balance before

        // TODO: Do mock bank message from treasury to admin account

        // Execute execute_propose (proposal, voting and execution in one single workflow)
        println!("Test Tube - signer/Admin address: {:?}", admin.account_id());
        let _execute_propose_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposeMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    msgs: vec![], // TODO: Mock a simple bank transfer that in prod will be the trigger exec to the middleware contract
                    proposer: None, // TODO: Some(admin.address()) is causing "pre-propose modules must specify a proposer. lacking one, no proposer should be specified: execute wasm contract failed"
                    vote_signatures,
                }),
                &vec![],
                &admin,
            )
            .unwrap();

        // TODO: Assert Admin balance after = (before + transfer_amount)

        // TODO: Assert proposal status after (closed, executed, deposit refunded, etc)
    }

    #[test]
    #[ignore]
    fn test_secp256k1_verify() {
        let (_app, _contracts, _admin, voters) = test_init(100);
        let deps = mock_dependencies();

        for voter in voters {
            let public_key = voter.public_key();
            let clear_message = b"Hello World";
            let message_hash = compute_sha256_hash(clear_message);
            let signature = voter.signing_key().sign(clear_message).unwrap();

            let verified = deps
                .api
                .secp256k1_verify(
                    message_hash.as_slice(),
                    signature.as_ref(),
                    public_key.to_bytes().as_ref(),
                )
                .expect("Invalid signature");

            assert!(verified == true);
        }
    }
}
