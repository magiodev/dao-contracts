#[cfg(test)]
pub mod test_tube {
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{to_json_binary, to_json_string, Api, BankMsg, Coin, CosmosMsg, Uint128};
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
    use osmosis_test_tube::RunnerError::ExecuteError;
    use osmosis_test_tube::{Account, Bank};
    use osmosis_test_tube::{Module, OsmosisTestApp, SigningAccount, Wasm};
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::contract::{compute_sha256_hash, create_adr36_message};
    use crate::msg::{
        ExecuteMsg, InstantiateMsg, ProposalPayload, SingleChoiceInstantProposalMsg, VoteSignature,
    };

    /// Init constants
    const SLUG_DAO_DAO_CORE: &str = "dao_dao_core";
    const SLUG_CW4_GROUP: &str = "cw4_group";
    const SLUG_DAO_VOTING_CW4: &str = "dao_voting_cw4";
    const SLUG_DAO_PROPOSAL_SINGLE_INSTANT: &str = "dao_proposal_single_instant";

    /// Test constants
    const INITIAL_BALANCE_AMOUNT: u128 = 1_000_000_000_000_000u128;
    const INITIAL_BALANCE_DENOM: &str = "ugov";
    pub const NONCE: &str = "123456";

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
            weight: 0,
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
            only_members_execute: true,
            allow_revoting: false,
            pre_propose_info: PreProposeInfo::AnyoneMayPropose {},
            close_proposal_on_execution_failure: true,
            veto: None,
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
                msg: to_json_binary(&prop_module_instantiate_msg).unwrap(),
                admin: Some(Admin::CoreModule {}),
                funds: vec![],
                label: "DAO DAO governance module".to_string(),
            }],
            voting_module_instantiate_info: ModuleInstantiateInfo {
                code_id: *code_ids.get(SLUG_DAO_VOTING_CW4).unwrap(),
                msg: to_json_binary(&vote_module_instantiate_msg).unwrap(),
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

        // Get Admin balance before send
        let bank_send_amount = 1000u128;
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

        // Create proposal execute msg as bank message from treasury back to the admin account
        let execute_propose_msg_pass: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
            to_address: admin.address(),
            amount: vec![Coin {
                denom: INITIAL_BALANCE_DENOM.to_string(),
                amount: Uint128::new(bank_send_amount),
            }],
        });

        let execute_propose_msg_fail: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
            to_address: admin.address(),
            amount: vec![Coin {
                denom: INITIAL_BALANCE_DENOM.to_string(),
                amount: Uint128::new(bank_send_amount * 2),
            }],
        });

        // Creating different messages for each voter.
        // The number of items of this array should match the test_init({voters_number}) value.
        let payloads: Vec<ProposalPayload> = vec![
            // A <- will pass! 1/2 majority
            ProposalPayload {
                msgs: vec![execute_propose_msg_pass.clone()],
                nonce: NONCE.to_string(),
            },
            // A <- will pass! 2/2 majority
            ProposalPayload {
                msgs: vec![execute_propose_msg_pass.clone()],
                nonce: NONCE.to_string(),
            },
            // B <- all the rest doesnt count
            ProposalPayload {
                msgs: vec![execute_propose_msg_fail.clone()],
                nonce: NONCE.to_string(),
            },
            ProposalPayload {
                msgs: vec![execute_propose_msg_fail.clone()],
                nonce: NONCE.to_string(),
            },
            ProposalPayload {
                msgs: vec![execute_propose_msg_fail.clone()],
                nonce: NONCE.to_string(),
            },
        ];

        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for (index, voter) in voters.iter().enumerate() {
            // Ensure that there's a message for each voter
            if let Some(clear_message) = payloads.get(index) {
                let clear_message_adr = create_adr36_message(
                    &to_json_string(&clear_message).unwrap(),
                    &voter.address(),
                );
                let message_hash = compute_sha256_hash(clear_message_adr.as_bytes());
                let signature = voter
                    .signing_key()
                    .sign(clear_message_adr.as_bytes())
                    .unwrap();
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
                &ExecuteMsg::Propose(SingleChoiceInstantProposalMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    payload: ProposalPayload {
                        msgs: vec![execute_propose_msg_pass.clone()],
                        nonce: NONCE.to_string(),
                    },
                    proposer: None,
                    vote_signatures,
                }),
                &vec![],
                &admin,
            )
            .unwrap();

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
    /// Test case of a proposal failing due to not be reaching the minimum members quorum.
    fn test_dao_proposal_single_instant_ko_not_quorum() {
        let (app, contracts, admin, voters) = test_init(2);
        let wasm = Wasm::new(&app);

        // Create proposal execute msg as bank message from treasury back to the admin account
        let bank_send_amount = 1000u128;
        let execute_propose_msg: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
            to_address: admin.address(),
            amount: vec![Coin {
                denom: INITIAL_BALANCE_DENOM.to_string(),
                amount: Uint128::new(bank_send_amount * 5),
            }],
        });

        // Creating different messages for each voter.
        // The number of items of this array should match the test_init({voters_number}) value.
        let messages: Vec<&CosmosMsg> = vec![&execute_propose_msg];

        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for (index, voter) in voters.iter().enumerate() {
            // Ensure that there's a message for each voter
            if let Some(clear_message) = messages.get(index) {
                let clear_message_adr = create_adr36_message(
                    &to_json_string(&clear_message).unwrap(),
                    &voter.address(),
                );
                let message_hash = compute_sha256_hash(clear_message_adr.as_bytes());
                let signature = voter
                    .signing_key()
                    .sign(clear_message_adr.as_bytes())
                    .unwrap();
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
        let execute_propose_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposalMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    payload: ProposalPayload {
                        msgs: vec![execute_propose_msg.clone()],
                        nonce: NONCE.to_string(),
                    },
                    proposer: None,
                    vote_signatures,
                }),
                &vec![],
                &admin,
            )
            .unwrap_err();

        // Assert that the response is an error of a specific type
        assert!(
            matches!(execute_propose_resp, ExecuteError { msg } if msg.contains("failed to execute message; message index: 0: proposal is not in 'passed' state: execute wasm contract failed"))
        );
    }

    #[test]
    #[ignore]
    /// Test case of a proposal failing due to be proposed by the a member of the same validator set, without passing trough the 0 voting power proposer role.
    fn test_dao_proposal_single_instant_ko_proposer() {
        let (app, contracts, admin, voters) = test_init(3);
        let wasm = Wasm::new(&app);

        // Create proposal execute msg as bank message from treasury back to the admin account
        let bank_send_amount = 1000u128;
        let execute_propose_msg: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
            to_address: admin.address(),
            amount: vec![Coin {
                denom: INITIAL_BALANCE_DENOM.to_string(),
                amount: Uint128::new(bank_send_amount * 5),
            }],
        });

        // Creating different messages for each voter.
        // The number of items of this array should match the test_init({voters_number}) value.
        let messages: Vec<&CosmosMsg> = vec![
            &execute_propose_msg,
            &execute_propose_msg,
            &execute_propose_msg,
        ];

        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for (index, voter) in voters.iter().enumerate() {
            // Ensure that there's a message for each voter
            if let Some(clear_message) = messages.get(index) {
                let clear_message_adr = create_adr36_message(
                    &to_json_string(&clear_message).unwrap(),
                    &voter.address(),
                );
                let message_hash = compute_sha256_hash(clear_message_adr.as_bytes());
                let signature = voter
                    .signing_key()
                    .sign(clear_message_adr.as_bytes())
                    .unwrap();
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
        let execute_propose_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposalMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    payload: ProposalPayload {
                        msgs: vec![execute_propose_msg.clone()],
                        nonce: NONCE.to_string(),
                    },
                    proposer: None,
                    vote_signatures,
                }),
                &vec![],
                &voters.get(0).unwrap(), // using first voter instead of admin to vote as member with voting power > 0
            )
            .unwrap_err();

        // Assert that the response is an error of a specific type (Unauthorized)
        assert!(
            matches!(execute_propose_resp, ExecuteError { msg } if msg.contains("failed to execute message; message index: 0: unauthorized: execute wasm contract failed"))
        );
    }

    #[test]
    #[ignore]
    /// Test case of a proposal failing due to be replayed
    fn test_dao_proposal_single_instant_ko_replay() {
        let (app, contracts, admin, voters) = test_init(2);
        let bank = Bank::new(&app);
        let wasm = Wasm::new(&app);

        // Execute bank send from admin to treasury, this is just a preparation step to allow the proposal to be executed.
        let bank_send_amount = 1000u128;
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

        // Create proposal execute msg as bank message from treasury back to the admin account
        let execute_propose_msg_pass: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
            to_address: admin.address(),
            amount: vec![Coin {
                denom: INITIAL_BALANCE_DENOM.to_string(),
                amount: Uint128::new(bank_send_amount),
            }],
        });

        // Creating different messages for each voter.
        // The number of items of this array should match the test_init({voters_number}) value.
        let payloads: Vec<ProposalPayload> = vec![
            // A <- will pass! 1/2 majority
            ProposalPayload {
                msgs: vec![execute_propose_msg_pass.clone()],
                nonce: NONCE.to_string(),
            },
            // A <- will pass! 2/2 majority
            ProposalPayload {
                msgs: vec![execute_propose_msg_pass.clone()],
                nonce: NONCE.to_string(),
            },
        ];

        let mut vote_signatures: Vec<VoteSignature> = vec![];
        for (index, voter) in voters.iter().enumerate() {
            // Ensure that there's a message for each voter
            if let Some(clear_message) = payloads.get(index) {
                let clear_message_adr = create_adr36_message(
                    &to_json_string(&clear_message).unwrap(),
                    &voter.address(),
                );
                let message_hash = compute_sha256_hash(clear_message_adr.as_bytes());
                let signature = voter
                    .signing_key()
                    .sign(clear_message_adr.as_bytes())
                    .unwrap();
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
        // This is the first, legit, execution of the proposal. We expect it to be succeed.
        wasm.execute(
            contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
            &ExecuteMsg::Propose(SingleChoiceInstantProposalMsg {
                title: "Title".to_string(),
                description: "Description".to_string(),
                payload: ProposalPayload {
                    msgs: vec![execute_propose_msg_pass.clone()],
                    nonce: NONCE.to_string(),
                },
                proposer: None,
                vote_signatures: vote_signatures.clone(),
            }),
            &vec![],
            &admin,
        )
        .unwrap();

        // Execute again execute_propose using the same payload.
        // This time we expect the proposal to fail due to replay of the nonce previously used.
        let execute_proposal_resp = wasm
            .execute(
                contracts.get(SLUG_DAO_PROPOSAL_SINGLE_INSTANT).unwrap(),
                &ExecuteMsg::Propose(SingleChoiceInstantProposalMsg {
                    title: "Title".to_string(),
                    description: "Description".to_string(),
                    payload: ProposalPayload {
                        msgs: vec![execute_propose_msg_pass.clone()],
                        nonce: NONCE.to_string(),
                    },
                    proposer: None,
                    vote_signatures,
                }),
                &vec![],
                &admin,
            )
            .unwrap_err();
        // Assert the error contains "nonce has been already used"
        assert!(
            matches!(execute_proposal_resp, ExecuteError { msg } if msg.contains("nonce has been already used"))
        );
    }

    #[test]
    #[ignore]
    fn test_secp256k1_verify() {
        let (_app, _contracts, _admin, voters) = test_init(10);
        let deps = mock_dependencies();

        for voter in voters {
            let message: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
                to_address: voter.address(),
                amount: vec![Coin {
                    denom: INITIAL_BALANCE_DENOM.to_string(),
                    amount: Uint128::new(1000u128),
                }],
            });

            let clear_message_string =
                create_adr36_message(&to_json_string(&message).unwrap(), &voter.address());
            let clear_message = compute_sha256_hash(clear_message_string.as_bytes());
            let signature = voter.signing_key().sign(clear_message.as_slice()).unwrap();

            // Verification
            let message_hash = compute_sha256_hash(clear_message.as_slice());
            let verified = deps
                .api
                .secp256k1_verify(
                    message_hash.as_slice(),
                    signature.as_ref(),
                    voter.public_key().to_bytes().as_ref(),
                )
                .expect("Invalid signature");

            assert!(verified == true);
        }
    }
}
