#[cfg(test)]
pub mod test_tube {
    use crate::contract::compute_sha256_hash;
    use crate::msg::{ExecuteMsg, InstantiateMsg, SingleChoiceInstantProposeMsg};
    use crate::state::{VoteMsg, VoteSignature};
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{to_binary, Api, BankMsg, Coin, CosmosMsg, Uint128};
    use cw_utils::Duration;
    use dao_interface::msg::InstantiateMsg as InstantiateMsgCore;
    use dao_interface::state::Admin;
    use dao_interface::state::ModuleInstantiateInfo;
    use dao_voting::pre_propose::PreProposeInfo;
    use dao_voting::threshold::Threshold;
    use dao_voting_cw4::msg::GroupContract;
    use osmosis_test_tube::osmosis_std::types::cosmos::bank::v1beta1::{
        MsgSend, QueryBalanceRequest,
    };
    use osmosis_test_tube::osmosis_std::types::cosmos::base::v1beta1;
    use osmosis_test_tube::RunnerError::{self, ExecuteError};
    use osmosis_test_tube::{Account, Bank};
    use osmosis_test_tube::{Module, OsmosisTestApp, SigningAccount, Wasm};
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Init constants
    const SLUG_DAO_DAO_CORE: &str = "dao_dao_core";
    const SLUG_CW4_GROUP: &str = "cw4_group";
    const SLUG_DAO_VOTING_CW4: &str = "dao_voting_cw4";
    const SLUG_DAO_PROPOSAL_SINGLE_INSTANT: &str = "dao_proposal_single_instant";

    /// Test constants
    const INITIAL_BALANCE_AMOUNT: u128 = 1_000_000_000_000_000u128;
    const INITIAL_BALANCE_DENOM: &str = "ugov";

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
            .init_account(&[
                Coin::new(INITIAL_BALANCE_AMOUNT, "uosmo"),
                Coin::new(INITIAL_BALANCE_AMOUNT, INITIAL_BALANCE_DENOM),
            ])
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
        let mut initial_members = voters
            .iter()
            .map(|voter| cw4::Member {
                addr: voter.address().to_string(),
                weight: 1,
            })
            .collect::<Vec<_>>();
        // Pushing proposer weight 0 account
        initial_members.push(cw4::Member {
            addr: admin.address().to_string(),
            weight: 1,
        });

        // Contracts to store and instantiate
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

        // Instantiate contract and sub-contracts
        // https://github.com/DA0-DA0/dao-contracts/wiki/Instantiating-a-DAO#proposal-module-instantiate-message
        let vote_module_instantiate_msg = dao_voting_cw4::msg::InstantiateMsg {
            group_contract: GroupContract::New {
                cw4_group_code_id: *code_ids.get(SLUG_CW4_GROUP).unwrap(),
                initial_members,
            },
        };
        let prop_module_instantiate_msg = InstantiateMsg {
            threshold: Threshold::AbsoluteCount {
                threshold: Uint128::new(2u128),
            },
            // TODO: Create an additional test variant as below
            // threshold: Threshold::ThresholdQuorum {
            //     threshold: PercentageThreshold,
            //     quorum: PercentageThreshold,
            // },
            max_voting_period: Duration::Height(1), // 1 block only to make it expire after the proposing block
            min_voting_period: None,
            only_members_execute: false, // TODO: Check if we want this at true, if yes we should remove the if power.is_zero() throw error from contract.rs
            allow_revoting: false,
            pre_propose_info: PreProposeInfo::AnyoneMayPropose {},
            close_proposal_on_execution_failure: true,
        };
        let dao_dao_core_instantiate_msg = InstantiateMsgCore {
            admin: Some(admin.address()),
            name: "DAO DAO Core".to_string(),
            description: "".to_string(),
            image_url: None,
            automatically_add_cw20s: true,
            automatically_add_cw721s: true,
            proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
                code_id: *code_ids.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                msg: to_binary(&prop_module_instantiate_msg).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "DAO DAO governance module".to_string(),
            }],
            voting_module_instantiate_info: ModuleInstantiateInfo {
                code_id: *code_ids.get(SLUG_DAO_VOTING_CW4).unwrap(),
                msg: to_binary(&vote_module_instantiate_msg).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "DAO DAO voting module".to_string(),
            },
            initial_items: None,
            dao_uri: None,
        };
        let dao_dao_core_instantiate_resp = wasm
            .instantiate(
                *code_ids.get(SLUG_DAO_DAO_CORE).unwrap(),
                &dao_dao_core_instantiate_msg,
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

        // Increase app time or members will not have any voting power assigned
        app.increase_time(10000);

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

    #[test]
    #[ignore]
    /// Test case of a proposal creation, voting passing and executing all-in-once, which should move gov funds from treasury.
    fn test_dao_proposal_single_instant_ok_send() {
        let (app, contracts, admin, voters) = test_init(5);
        let bank = Bank::new(&app);
        let wasm = Wasm::new(&app);

        // Create proposal execute msg as bank message from treasury back to the admin account
        let bank_send_amount = 1000u128;
        let execute_propose_msg = VoteMsg {
            nonce: "0123456789".to_string(),
            msg: CosmosMsg::Bank(BankMsg::Send {
                to_address: admin.address(),
                amount: vec![Coin {
                    denom: INITIAL_BALANCE_DENOM.to_string(),
                    amount: Uint128::new(bank_send_amount),
                }],
            }),
        };
        let execute_propose_msg_binary = to_binary(&execute_propose_msg).unwrap();

        // Creating different messages for each voter.
        // ... add as many messages as there are voters
        // The number of items of this array should match the test_init({voters_number}) value.
        let messages: Vec<&[u8]> = vec![
            execute_propose_msg_binary.as_slice(), // A <- will pass!
            execute_propose_msg_binary.as_slice(), // A <- will pass!
            execute_propose_msg_binary.as_slice(), // A <- will pass!
            b"Hello World!",                       // B
            b"Hello World!",                       // B
        ];

        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for (index, voter) in voters.iter().enumerate() {
            // Ensure that there's a message for each voter
            if let Some(clear_message) = messages.get(index) {
                let message_hash = compute_sha256_hash(clear_message);
                let signature = voter.signing_key().sign(clear_message).unwrap();
                // VoteSignature
                vote_signatures.push(VoteSignature {
                    message_hash,
                    signature: signature.as_ref().to_vec(),
                    public_key: voter.public_key().to_bytes(),
                });
            } else {
                // Do nothing in the case where there's no message for a voter
            }
        }

        // Get Admin balance before send
        let admin_balance_before = bank
            .query_balance(&QueryBalanceRequest {
                address: admin.address(),
                denom: INITIAL_BALANCE_DENOM.to_string(),
            })
            .unwrap()
            .balance
            .expect("failed to query balance");

        // Execute bank send from admin to treasury
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contracts
                    .get(SLUG_DAO_DAO_CORE)
                    .expect("Treasury address not found")
                    .clone(),
                amount: vec![v1beta1::Coin {
                    denom: INITIAL_BALANCE_DENOM.to_string(),
                    amount: bank_send_amount.to_string(),
                }],
            },
            &admin,
        )
        .unwrap();

        // Get Admin balance after send
        let admin_balance_after_send = bank
            .query_balance(&QueryBalanceRequest {
                address: admin.address(),
                denom: INITIAL_BALANCE_DENOM.to_string(),
            })
            .unwrap()
            .balance
            .expect("failed to query balance");
        let admin_balance_after = admin_balance_after_send
            .amount
            .parse::<u128>()
            .expect("Failed to parse after balance");
        let admin_balance_before = admin_balance_before
            .amount
            .parse::<u128>()
            .expect("Failed to parse before balance");
        assert!(admin_balance_after == admin_balance_before - bank_send_amount);

        // Get treasury balance after send
        let treasury_balance_after_send = bank
            .query_balance(&QueryBalanceRequest {
                address: contracts
                    .get(SLUG_DAO_DAO_CORE)
                    .expect("Treasury address not found")
                    .clone(),
                denom: INITIAL_BALANCE_DENOM.to_string(),
            })
            .unwrap()
            .balance
            .expect("failed to query balance");
        let treasury_balance_after = treasury_balance_after_send
            .amount
            .parse::<u128>()
            .expect("Failed to parse after balance");
        assert!(treasury_balance_after == bank_send_amount);

        // Execute execute_propose (proposal, voting and execution in one single workflow)
        let _execute_propose_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposeMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    msgs: vec![execute_propose_msg],
                    proposer: None,
                    vote_signatures,
                }),
                &vec![],
                &admin,
            )
            .unwrap();

        // TODO: Assert votes, execution and proposal status from response attributes.

        // Get Admin balance after proposal
        let admin_balance_after_proposal = bank
            .query_balance(&QueryBalanceRequest {
                address: admin.address(),
                denom: INITIAL_BALANCE_DENOM.to_string(),
            })
            .unwrap()
            .balance
            .expect("failed to query balance");
        let admin_balance_after = admin_balance_after_proposal
            .amount
            .parse::<u128>()
            .expect("Failed to parse after balance");
        assert!(admin_balance_after == admin_balance_before);
    }

    #[test]
    #[ignore]
    /// Test case of a proposal creation, voting passing and executing all-in-once, which should move gov funds from treasury.
    fn test_dao_proposal_single_instant_ko_replay() {
        let (app, contracts, admin, voters) = test_init(5);
        let bank = Bank::new(&app);
        let wasm = Wasm::new(&app);

        // Create proposal execute msg as bank message from treasury back to the admin account
        let bank_send_amount = 1000u128;
        let mut execute_propose_msg = VoteMsg {
            nonce: "0123456789".to_string(),
            msg: CosmosMsg::Bank(BankMsg::Send {
                to_address: admin.address(),
                amount: vec![Coin {
                    denom: INITIAL_BALANCE_DENOM.to_string(),
                    amount: Uint128::new(bank_send_amount),
                }],
            }),
        };
        let execute_propose_msg_binary = to_binary(&execute_propose_msg).unwrap();

        // Creating different messages for each voter.
        // ... add as many messages as there are voters
        // The number of items of this array should match the test_init({voters_number}) value.
        let messages: Vec<&[u8]> = vec![
            execute_propose_msg_binary.as_slice(), // A <- will pass!
            execute_propose_msg_binary.as_slice(), // A <- will pass!
            execute_propose_msg_binary.as_slice(), // A <- will pass!
            b"Hello World!",                       // B
            b"Hello World!",                       // B
        ];

        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for (index, voter) in voters.iter().enumerate() {
            // Ensure that there's a message for each voter
            if let Some(clear_message) = messages.get(index) {
                let message_hash = compute_sha256_hash(clear_message);
                let signature = voter.signing_key().sign(clear_message).unwrap();
                // VoteSignature
                vote_signatures.push(VoteSignature {
                    message_hash,
                    signature: signature.as_ref().to_vec(),
                    public_key: voter.public_key().to_bytes(),
                });
            } else {
                // Do nothing in the case where there's no message for a voter
            }
        }

        // Execute bank send from admin to treasury
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contracts
                    .get(SLUG_DAO_DAO_CORE)
                    .expect("Treasury address not found")
                    .clone(),
                amount: vec![v1beta1::Coin {
                    denom: INITIAL_BALANCE_DENOM.to_string(),
                    amount: (bank_send_amount * 2).to_string(), // we are senging the amount times 2
                }],
            },
            &admin,
        )
        .unwrap();

        // Get treasury balance after send
        let treasury_balance_after_send = bank
            .query_balance(&QueryBalanceRequest {
                address: contracts
                    .get(SLUG_DAO_DAO_CORE)
                    .expect("Treasury address not found")
                    .clone(),
                denom: INITIAL_BALANCE_DENOM.to_string(),
            })
            .unwrap()
            .balance
            .expect("failed to query balance");
        let treasury_balance_after = treasury_balance_after_send
            .amount
            .parse::<u128>()
            .expect("Failed to parse after balance");
        assert_eq!(treasury_balance_after, bank_send_amount * 2);

        // Execute execute_propose (proposal, voting and execution in one single workflow)
        let _execute_propose_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposeMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    msgs: vec![execute_propose_msg.clone()],
                    proposer: None,
                    vote_signatures: vote_signatures.clone(),
                }),
                &vec![],
                &admin,
            )
            .unwrap();

        // Get treasury balance after first execution
        let treasury_balance_after_replay = bank
            .query_balance(&QueryBalanceRequest {
                address: contracts
                    .get(SLUG_DAO_DAO_CORE)
                    .expect("Treasury address not found")
                    .clone(),
                denom: INITIAL_BALANCE_DENOM.to_string(),
            })
            .unwrap()
            .balance
            .expect("failed to query balance");
        let treasury_balance_after = treasury_balance_after_replay
            .amount
            .parse::<u128>()
            .expect("Failed to parse after balance");
        assert_eq!(treasury_balance_after, bank_send_amount);

        // Try to replay with the same exact payload
        let execute_propose_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposeMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    msgs: vec![execute_propose_msg.clone()],
                    proposer: None,
                    vote_signatures: vote_signatures.clone(),
                }),
                &vec![],
                &admin,
            )
            .unwrap_err();

        // Assert that the response is an error of a specific type (Unauthorized)
        assert!(
            matches!(execute_propose_resp, ExecuteError { msg } if msg.contains("failed to execute message; message index: 0: unauthorized: execute wasm contract failed"))
        );

        // Get treasury balance after replay
        // here we expect the same balance of before, as the second execution should have been prevented
        let treasury_balance_after_replay = bank
            .query_balance(&QueryBalanceRequest {
                address: contracts
                    .get(SLUG_DAO_DAO_CORE)
                    .expect("Treasury address not found")
                    .clone(),
                denom: INITIAL_BALANCE_DENOM.to_string(),
            })
            .unwrap()
            .balance
            .expect("failed to query balance");
        let treasury_balance_after = treasury_balance_after_replay
            .amount
            .parse::<u128>()
            .expect("Failed to parse after balance");
        assert_eq!(treasury_balance_after, bank_send_amount);

        // Try to replay by altering the nonce and so skipping the first validation, but same VoteSignatures
        execute_propose_msg.nonce = "9876543210".to_string();
        let execute_propose_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposeMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    msgs: vec![execute_propose_msg],
                    proposer: None,
                    vote_signatures: vote_signatures,
                }),
                &vec![],
                &admin,
            )
            .unwrap_err();

        // Assert that the response is an error of a specific type (MessageHashMismatch)
        assert!(
            matches!(execute_propose_resp, ExecuteError { msg } if msg.contains("failed to execute message; message index: 0: majority execute message hash mismatch: execute wasm contract failed"))
        );

        // Get treasury balance after replay
        // here we expect the same balance of before, as the second execution should have been prevented
        let treasury_balance_after_replay = bank
            .query_balance(&QueryBalanceRequest {
                address: contracts
                    .get(SLUG_DAO_DAO_CORE)
                    .expect("Treasury address not found")
                    .clone(),
                denom: INITIAL_BALANCE_DENOM.to_string(),
            })
            .unwrap()
            .balance
            .expect("failed to query balance");
        let treasury_balance_after = treasury_balance_after_replay
            .amount
            .parse::<u128>()
            .expect("Failed to parse after balance");
        assert_eq!(treasury_balance_after, bank_send_amount);
    }

    #[test]
    #[ignore]
    /// TODO: Test case of a proposal failing due to a tie in message_hash_majority computation by voting_power.
    fn test_dao_proposal_single_instant_ko_tie() {
        let (app, contracts, admin, voters) = test_init(5);
        let wasm = Wasm::new(&app);

        // Creating different messages for each voter.
        // The number of items of this array should match the test_init({voters_number}) value.
        let messages: Vec<&[u8]> = vec![
            b"Hello World! 0",
            b"Hello World! 1",
            b"Hello World! 2",
            b"Hello World! 3",
            b"Hello World! 4",
            // ... add as many messages as there are voters
        ];

        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for (index, voter) in voters.iter().enumerate() {
            // Ensure that there's a message for each voter
            if let Some(clear_message) = messages.get(index) {
                let message_hash = compute_sha256_hash(clear_message);
                let signature = voter.signing_key().sign(clear_message).unwrap();

                // VoteSignature
                vote_signatures.push(VoteSignature {
                    message_hash,
                    signature: signature.as_ref().to_vec(),
                    public_key: voter.public_key().to_bytes(),
                });
            } else {
                // Do nothing in the case where there's no message for a voter
            }
        }

        // Execute execute_propose (proposal, voting and execution in one single workflow)
        let _execute_propose_resp = wasm.execute(
            contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
            &ExecuteMsg::Propose(SingleChoiceInstantProposeMsg {
                title: "Title".to_string(),
                description: "Description".to_string(),
                msgs: vec![],
                proposer: None,
                vote_signatures,
            }),
            &vec![],
            &admin,
        );

        match _execute_propose_resp {
            Ok(_) => panic!(
                "Expected an error for not reaching the threshold, but the operation succeeded"
            ),
            Err(e) => {
                // Check if the error is the expected one
                let error_message = format!("{:?}", e);
                assert!(
                    error_message.contains("Not possible to reach required (passing) threshold"),
                    "Unexpected error message: {}",
                    error_message
                );
            }
        }
    }

    #[test]
    #[ignore]
    /// TODO: Test case of a proposal failing due to not be reaching the minimum members quorum.
    fn test_dao_proposal_single_instant_ko_not_quorum() {
        let (app, contracts, admin, voters) = test_init(2);
        let wasm = Wasm::new(&app);

        // Creating different messages for each voter.
        // The number of items of this array should match the test_init({voters_number}) value.
        let messages: Vec<&[u8]> = vec![
            b"Hello World!", // only one vote when 2 is required on test_init() fixture
            b"Hello World!",
        ];

        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for (index, voter) in voters.iter().enumerate() {
            // Ensure that there's a message for each voter
            if let Some(clear_message) = messages.get(index) {
                let message_hash = compute_sha256_hash(clear_message);
                let signature = voter.signing_key().sign(clear_message).unwrap();

                // VoteSignature
                vote_signatures.push(VoteSignature {
                    message_hash,
                    signature: signature.as_ref().to_vec(),
                    public_key: voter.public_key().to_bytes(),
                });
            } else {
                // Do nothing in the case where there's no message for a voter
            }
        }

        // Execute execute_propose (proposal, voting and execution in one single workflow)
        let _execute_propose_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposeMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    msgs: vec![],
                    proposer: None,
                    vote_signatures,
                }),
                &vec![],
                &admin,
            )
            .unwrap();
        // TODO: Assert error called `Result::unwrap()` on an `Err` value: ExecuteError { msg: "failed to execute message; message index: 0: proposal is not in 'passed' state: execute wasm contract failed" }
    }

    #[test]
    #[ignore]
    /// TODO: Test case of a proposal failing due to be proposed by the a member of the same validator set, without passing trough the 0 voting power proposer role.
    fn test_dao_proposal_single_instant_ko_proposer() {
        let (app, contracts, _admin, voters) = test_init(3);
        let wasm = Wasm::new(&app);

        // Creating different messages for each voter.
        // The number of items of this array should match the test_init({voters_number}) value.
        let messages: Vec<&[u8]> = vec![b"Hello World!", b"Hello World!", b"Hello World!"];

        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for (index, voter) in voters.iter().enumerate() {
            // Ensure that there's a message for each voter
            if let Some(clear_message) = messages.get(index) {
                let message_hash = compute_sha256_hash(clear_message);
                let signature = voter.signing_key().sign(clear_message).unwrap();

                // VoteSignature
                vote_signatures.push(VoteSignature {
                    message_hash,
                    signature: signature.as_ref().to_vec(),
                    public_key: voter.public_key().to_bytes(),
                });
            } else {
                // Do nothing in the case where there's no message for a voter
            }
        }

        // Execute execute_propose (proposal, voting and execution in one single workflow)
        let _execute_propose_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposeMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    msgs: vec![],
                    proposer: None,
                    vote_signatures,
                }),
                &vec![],
                &voters.get(0).unwrap(), // using first voter instead of admin
            )
            .unwrap();
        // TODO: Assert error unauthorized
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
