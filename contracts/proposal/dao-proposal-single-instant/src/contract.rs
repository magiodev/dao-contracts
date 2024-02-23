use crate::msg::SingleChoiceInstantProposalMsg as ProposeMsg;
use bech32::ToBase32;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Order, Reply,
    Response, StdResult, Storage, SubMsg, Uint128, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version, ContractVersion};
use cw4::MemberListResponse;
use cw4_group::msg::QueryMsg::ListMembers;
use cw_hooks::Hooks;
use cw_storage_plus::Bound;
use cw_utils::{parse_reply_instantiate_data, Duration};
use dao_hooks::proposal::{
    new_proposal_hooks, proposal_completed_hooks, proposal_status_changed_hooks,
};
use dao_hooks::vote::new_vote_hooks;
use dao_interface::voting::IsActiveResponse;
use dao_voting::pre_propose::{PreProposeInfo, ProposalCreationPolicy};
use dao_voting::proposal::{DEFAULT_LIMIT, MAX_PROPOSAL_SIZE};
use dao_voting::reply::{
    failed_pre_propose_module_hook_id, mask_proposal_execution_proposal_id, TaggedReplyId,
};
use dao_voting::status::Status;
use dao_voting::threshold::{Threshold, ThresholdError};
use dao_voting::veto::{VetoConfig, VetoError};
use dao_voting::voting::{get_total_power, get_voting_power, validate_voting_period, Vote, Votes};
use dao_voting_cw4::msg::QueryMsg::GroupContract;
use ripemd::{Digest as RipDigest, Ripemd160};
use sha2::Sha256;
use std::collections::HashMap;
use std::convert::TryInto;

use crate::msg::MigrateMsg;
use crate::proposal::{next_proposal_id, SingleChoiceProposal};
use crate::state::{Config, VoteSignature, CREATION_POLICY};
use crate::v1_state::{
    v1_duration_to_v2, v1_expiration_to_v2, v1_status_to_v2, v1_threshold_to_v2, v1_votes_to_v2,
};
use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    proposal::advance_proposal_id,
    query::ProposalListResponse,
    query::{ProposalResponse, VoteInfo, VoteListResponse, VoteResponse},
    state::{Ballot, BALLOTS, CONFIG, PROPOSALS, PROPOSAL_COUNT, PROPOSAL_HOOKS, VOTE_HOOKS},
};
use cw_proposal_single_v1 as v1;
pub(crate) const CONTRACT_NAME: &str = "crates.io:dao-proposal-single-instant";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    msg.threshold.validate()?;

    let dao = info.sender;

    let (min_voting_period, max_voting_period) =
        validate_voting_period(msg.min_voting_period, msg.max_voting_period)?;

    let (initial_policy, pre_propose_messages) = msg
        .pre_propose_info
        .into_initial_policy_and_messages(dao.clone())?;

    // if veto is configured, validate its fields
    if let Some(veto_config) = &msg.veto {
        veto_config.validate(&deps.as_ref(), &max_voting_period)?;
    };

    let config = Config {
        threshold: msg.threshold,
        max_voting_period,
        min_voting_period,
        only_members_execute: msg.only_members_execute,
        dao: dao.clone(),
        allow_revoting: msg.allow_revoting,
        close_proposal_on_execution_failure: msg.close_proposal_on_execution_failure,
        veto: msg.veto,
    };

    // Initialize proposal count to zero so that queries return zero
    // instead of None.
    PROPOSAL_COUNT.save(deps.storage, &0)?;
    CONFIG.save(deps.storage, &config)?;
    CREATION_POLICY.save(deps.storage, &initial_policy)?;

    Ok(Response::default()
        .add_submessages(pre_propose_messages)
        .add_attribute("action", "instantiate")
        .add_attribute("dao", dao))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Propose(ProposeMsg {
            title,
            description,
            msgs,
            proposer,
            vote_signatures,
        }) => execute_propose(
            deps,
            env,
            info,
            title,
            description,
            msgs,
            proposer,
            vote_signatures,
        ),
        ExecuteMsg::UpdateRationale {
            proposal_id,
            rationale,
        } => execute_update_rationale(deps, info, proposal_id, rationale),
        ExecuteMsg::Close { proposal_id } => execute_close(deps, env, info, proposal_id),
        ExecuteMsg::UpdateConfig {
            threshold,
            max_voting_period,
            min_voting_period,
            only_members_execute,
            allow_revoting,
            dao,
            close_proposal_on_execution_failure,
            veto,
        } => execute_update_config(
            deps,
            info,
            threshold,
            max_voting_period,
            min_voting_period,
            only_members_execute,
            allow_revoting,
            dao,
            close_proposal_on_execution_failure,
            veto,
        ),
        ExecuteMsg::UpdatePreProposeInfo { info: new_info } => {
            execute_update_proposal_creation_policy(deps, info, new_info)
        }
        ExecuteMsg::AddProposalHook { address } => {
            execute_add_proposal_hook(deps, env, info, address)
        }
        ExecuteMsg::RemoveProposalHook { address } => {
            execute_remove_proposal_hook(deps, env, info, address)
        }
        ExecuteMsg::AddVoteHook { address } => execute_add_vote_hook(deps, env, info, address),
        ExecuteMsg::RemoveVoteHook { address } => {
            execute_remove_vote_hook(deps, env, info, address)
        }
    }
}

pub fn execute_propose(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    title: String,
    description: String,
    msgs: Vec<CosmosMsg<Empty>>,
    proposer: Option<String>,
    vote_signatures: Vec<VoteSignature>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let proposal_creation_policy = CREATION_POLICY.load(deps.storage)?;

    // Check that the sender is permitted to create proposals.
    if !proposal_creation_policy.is_permitted(&info.sender) {
        return Err(ContractError::Unauthorized {});
    }

    // MVP Limitation: Supporting only one msg per proposal
    if msgs.len() > 1 {
        return Err(ContractError::TooManyMsgs {});
    }

    // Determine the appropriate proposer. If this is coming from our
    // pre-propose module, it must be specified. Otherwise, the
    // proposer should not be specified.
    let proposer = match (proposer, &proposal_creation_policy) {
        (None, ProposalCreationPolicy::Anyone {}) => info.sender.clone(),
        // `is_permitted` above checks that an allowed module is
        // actually sending the propose message.
        (Some(proposer), ProposalCreationPolicy::Module { .. }) => {
            deps.api.addr_validate(&proposer)?
        }
        _ => return Err(ContractError::InvalidProposer {}),
    };

    let voting_module: Addr = deps.querier.query_wasm_smart(
        config.dao.clone(),
        &dao_interface::msg::QueryMsg::VotingModule {},
    )?;

    // Voting modules are not required to implement this
    // query. Lacking an implementation they are active by default.
    let active_resp: IsActiveResponse = deps
        .querier
        .query_wasm_smart(voting_module, &dao_interface::voting::Query::IsActive {})
        .unwrap_or(IsActiveResponse { active: true });

    if !active_resp.active {
        return Err(ContractError::InactiveDao {});
    }

    let expiration = config.max_voting_period.after(&env.block);

    let total_power = get_total_power(deps.as_ref(), &config.dao, Some(env.block.height))?;

    let proposal = {
        // Limit mutability to this block.
        let mut proposal = SingleChoiceProposal {
            title,
            description,
            proposer: proposer.clone(),
            start_height: env.block.height,
            min_voting_period: config.min_voting_period.map(|min| min.after(&env.block)),
            expiration,
            threshold: config.threshold,
            total_power,
            msgs: msgs.clone(),
            status: Status::Open,
            votes: Votes::zero(),
            allow_revoting: config.allow_revoting,
            veto: config.veto,
        };
        // Update the proposal's status. Addresses case where proposal
        // expires on the same block as it is created.
        proposal.update_status(&env.block)?;
        proposal
    };
    let id = advance_proposal_id(deps.storage)?;

    // Limit the size of proposals.
    //
    // The Juno mainnet has a larger limit for data that can be
    // uploaded as part of an execute message than it does for data
    // that can be queried as part of a query. This means that without
    // this check it is possible to create a proposal that can not be
    // queried.
    //
    // The size selected was determined by uploading versions of this
    // contract to the Juno mainnet until queries worked within a
    // reasonable margin of error.
    //
    // `to_vec` is the method used by cosmwasm to convert a struct
    // into it's byte representation in storage.
    let proposal_size = cosmwasm_std::to_json_vec(&proposal)?.len() as u64;
    if proposal_size > MAX_PROPOSAL_SIZE {
        return Err(ContractError::ProposalTooLarge {
            size: proposal_size,
            max: MAX_PROPOSAL_SIZE,
        });
    }

    PROPOSALS.save(deps.storage, id, &proposal)?;

    let hooks = new_proposal_hooks(PROPOSAL_HOOKS, deps.storage, id, proposer.as_str())?;

    // Init empty message hash majority counts, this will be filled with message hashes and their accrued voting power
    let mut message_hash_counts: HashMap<Vec<u8>, Uint128> = HashMap::new();

    // sum vote counts based on member weight / voting power
    for vote_signature in &vote_signatures {
        let voter_address = derive_addr_from_pubkey(&vote_signature.public_key, "osmo").unwrap();

        let vote_power = get_voting_power(
            deps.as_ref(),
            Addr::unchecked(voter_address),
            &config.dao,
            Some(proposal.start_height),
        )?;

        *message_hash_counts
            .entry(vote_signature.message_hash.clone())
            .or_insert(Uint128::zero()) += vote_power;
    }

    // Validate that message_hash_counts contains at least one key with value > 0
    if !message_hash_counts
        .values()
        .any(|&value| value > Uint128::zero())
    {
        return Err(ContractError::ThresholdError(
            ThresholdError::UnreachableThreshold {},
        ));
    }

    let mut p_vote_attributes = vec![];
    let mut p_vote_messages = vec![];

    // verify and cast votes
    for vote_signature in &vote_signatures {
        let (address, verified) = verify_message(deps.as_ref(), vote_signature)?;
        let voter_address = deps.api.addr_validate(address.as_str())?;

        // Checking if the current voter is a member with voting power higher than 0
        let voting_power = get_voting_power(
            deps.as_ref(),
            voter_address.clone(),
            &config.dao,
            Some(proposal.start_height),
        )?;
        let is_member = voting_power != Uint128::zero();

        // Match the message_hash wrapped by ADR36 SignDoc and signer address
        let proposal_msg = msgs.get(0).unwrap();
        let proposal_msg_adr36 = create_adr36_message(
            &serde_json_wasm::to_string(&proposal_msg).unwrap(),
            &address,
        );
        let proposal_message_hash = compute_sha256_hash(&proposal_msg_adr36.as_bytes());

        // If Signature has been verified and a Member address has been found
        if verified && is_member {
            // Compute yes or no vote based on majority previous computed.
            let vote = Some(if vote_signature.message_hash == proposal_message_hash {
                Vote::Yes
            } else {
                Vote::No
            });

            // Call proposal_vote only if vote_option is not None
            if let Some(vote) = vote {
                let mut p_vote = proposal_vote(
                    deps.branch(),
                    env.clone(),
                    info.clone(),
                    id,
                    voter_address,
                    vote,
                    None, // rationale hardcoded to None
                )?;
                p_vote_attributes.append(p_vote.attributes.as_mut());
                p_vote_messages.append(p_vote.messages.as_mut());
            }
        } else {
            // Do nothing, skip this iteration and continue. We didn't recognize the address on members list.
            continue;
        }
    }

    let p_execute = proposal_execute(deps.branch(), env, info.clone(), id)?;

    Ok(Response::default()
        .add_attributes(p_vote_attributes)
        .add_submessages(p_vote_messages)
        .add_attributes(p_execute.attributes)
        .add_submessages(p_execute.messages)
        .add_submessages(hooks)
        .add_attribute("action", "propose")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", id.to_string())
        .add_attribute("status", proposal.status.to_string()))
}

// This is veryfing the signature for a given publicKey and messageHash.
// In the context of this contract, is assumed that the signature is generated from an ADR36 compliant message
fn verify_message(
    deps: Deps,
    vote_signature: &VoteSignature,
) -> Result<(String, bool), ContractError> {
    let voter_address = derive_addr_from_pubkey(&vote_signature.public_key, "osmo")?;
    let verified = deps
        .api
        .secp256k1_verify(
            vote_signature.message_hash.as_slice(),
            vote_signature.signature.as_slice(),
            vote_signature.public_key.as_slice(),
        )
        .unwrap();

    Ok((voter_address, verified))
}

pub fn create_adr36_message(data: &String, signer: &String) -> String {
    let message = format!(
        "{{\"account_number\":\"0\",\"chain_id\":\"\",\"fee\":{{\"amount\":[],\"gas\":\"0\"}},\"memo\":\"\",\"msgs\":[{{\"type\":\"sign/MsgSignData\",\"value\":{{\"data\":\"{}\",\"signer\":\"{}\"}}}}],\"sequence\":\"0\"}}",
        base64::encode(data),
        signer
    );

    message
}

pub fn compute_sha256_hash(message: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(message);
    hasher.finalize().to_vec()
}

pub fn derive_addr_from_pubkey(pub_key: &[u8], hrp: &str) -> Result<String, ContractError> {
    let sha_hash: [u8; 32] = Sha256::digest(pub_key)
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::WrongLength {})?;

    let rip_hash = Ripemd160::digest(sha_hash);
    let rip_slice: &[u8] = rip_hash.as_slice();

    let addr: String = bech32::encode(hrp, rip_slice.to_base32(), bech32::Variant::Bech32)
        .map_err(|_| ContractError::VerificationFailed {})?;
    Ok(addr)
}

fn proposal_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    voter_address: Addr,
    vote: Vote,
    rationale: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut prop = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(ContractError::NoSuchProposal { id: proposal_id })?;

    // Allow voting on proposals until they expire.
    // Voting on a non-open proposal will never change
    // their outcome as if an outcome has been determined,
    // it is because no possible sequence of votes may
    // cause a different one. This then serves to allow
    // for better tallies of opinions in the event that a
    // proposal passes or is rejected early.
    if prop.expiration.is_expired(&env.block) {
        return Err(ContractError::Expired { id: proposal_id });
    }

    // we use voter_address instead of using info.sender
    let vote_power = get_voting_power(
        deps.as_ref(),
        voter_address.clone(),
        &config.dao,
        Some(prop.start_height),
    )?;

    if vote_power.is_zero() {
        return Err(ContractError::NotRegistered {});
    }

    BALLOTS.update(
        deps.storage,
        (proposal_id, &voter_address),
        |bal| match bal {
            Some(current_ballot) => {
                if prop.allow_revoting {
                    if current_ballot.vote == vote {
                        // Don't allow casting the same vote more than
                        // once. This seems liable to be confusing
                        // behavior.
                        Err(ContractError::AlreadyCast {})
                    } else {
                        // Remove the old vote if this is a re-vote.
                        prop.votes
                            .remove_vote(current_ballot.vote, current_ballot.power);
                        Ok(Ballot {
                            power: vote_power,
                            vote,
                            // Roll over the previous rationale. If
                            // you're changing your vote, you've also
                            // likely changed your thinking.
                            rationale: rationale.clone(),
                        })
                    }
                } else {
                    Err(ContractError::AlreadyVoted {})
                }
            }
            None => Ok(Ballot {
                power: vote_power,
                vote,
                rationale: rationale.clone(),
            }),
        },
    )?;

    let old_status = prop.status;

    prop.votes.add_vote(vote, vote_power);
    prop.update_status(&env.block)?;

    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    let new_status = prop.status;
    let change_hooks = proposal_status_changed_hooks(
        PROPOSAL_HOOKS,
        deps.storage,
        proposal_id,
        old_status.to_string(),
        new_status.to_string(),
    )?;

    let vote_hooks = new_vote_hooks(
        VOTE_HOOKS,
        deps.storage,
        proposal_id,
        voter_address.to_string(),
        vote.to_string(),
    )?;

    Ok(Response::default()
        .add_submessages(change_hooks)
        .add_submessages(vote_hooks)
        .add_attribute("action", "vote")
        .add_attribute("sender", info.sender)
        .add_attribute("voter_address", voter_address)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("position", vote.to_string())
        .add_attribute("rationale", rationale.as_deref().unwrap_or("_none"))
        .add_attribute("status", prop.status.to_string()))
}

fn proposal_execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let mut prop = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(ContractError::NoSuchProposal { id: proposal_id })?;

    let config = CONFIG.load(deps.storage)?;

    // determine if this sender can execute
    let mut sender_can_execute = true;
    if config.only_members_execute {
        // Get dao-voting-cw4 contract address
        let dao_voting_cw4_addr: Addr = deps.querier.query_wasm_smart(
            config.dao.clone(),
            &dao_interface::msg::QueryMsg::VotingModule {},
        )?;

        // Get cw4-group contract address
        let cw4_group_addr: Addr = deps
            .querier
            .query_wasm_smart(dao_voting_cw4_addr, &GroupContract {})?;

        // Get list of members
        let members: MemberListResponse = deps.querier.query_wasm_smart(
            cw4_group_addr.clone(),
            &ListMembers {
                start_after: None,
                limit: None,
            },
        )?;

        let is_member = members
            .members
            .iter()
            .any(|member| member.addr == info.sender);
        let power = get_voting_power(
            deps.as_ref(),
            info.sender.clone(),
            &config.dao,
            Some(prop.start_height),
        )?;

        // Proposer should be a member and voting weight should be zero
        sender_can_execute = is_member && power.is_zero();
    }

    // Check here that the proposal is passed or timelocked.
    // Allow it to be executed even if it is expired so long
    // as it passed during its voting period. Allow it to be
    // executed in timelock state if early_execute is enabled
    // and the sender is the vetoer.
    prop.update_status(&env.block)?;
    let old_status = prop.status;
    match &prop.status {
        Status::Passed => {
            // if passed, verify sender can execute
            if !sender_can_execute {
                return Err(ContractError::Unauthorized {});
            }
        }
        Status::VetoTimelock { .. } => {
            // should never error if in veto timelock state
            let veto_config = prop
                .veto
                .as_ref()
                .ok_or(VetoError::NoVetoConfiguration {})?;

            // check that the sender is the vetoer
            if veto_config.vetoer != info.sender {
                // if the sender can normally execute, but is not the vetoer,
                // return timelocked error. otherwise return unauthorized.
                if sender_can_execute {
                    return Err(ContractError::VetoError(VetoError::Timelocked {}));
                } else {
                    return Err(ContractError::Unauthorized {});
                }
            }

            // if veto timelocked, only allow execution if early_execute enabled
            veto_config.check_early_execute_enabled()?;
        }
        _ => {
            return Err(ContractError::NotPassed {});
        }
    }

    prop.status = Status::Executed;

    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    let response = {
        if !prop.msgs.is_empty() {
            let execute_message = WasmMsg::Execute {
                contract_addr: config.dao.to_string(),
                msg: to_json_binary(&dao_interface::msg::ExecuteMsg::ExecuteProposalHook {
                    msgs: prop.msgs,
                })?,
                funds: vec![],
            };
            match config.close_proposal_on_execution_failure {
                true => {
                    let masked_proposal_id = mask_proposal_execution_proposal_id(proposal_id);
                    Response::default()
                        .add_submessage(SubMsg::reply_on_error(execute_message, masked_proposal_id))
                }
                false => Response::default().add_message(execute_message),
            }
        } else {
            Response::default()
        }
    };

    // Add proposal status change hooks
    let proposal_status_changed_hooks = proposal_status_changed_hooks(
        PROPOSAL_HOOKS,
        deps.storage,
        proposal_id,
        old_status.to_string(),
        prop.status.to_string(),
    )?;

    // Add prepropose / deposit module hook which will handle deposit refunds.
    let proposal_creation_policy = CREATION_POLICY.load(deps.storage)?;
    let proposal_completed_hooks =
        proposal_completed_hooks(proposal_creation_policy, proposal_id, prop.status)?;

    Ok(response
        .add_submessages(proposal_status_changed_hooks)
        .add_submessages(proposal_completed_hooks)
        .add_attribute("action", "execute")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("dao", config.dao))
}

pub fn execute_veto(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let mut prop = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(ContractError::NoSuchProposal { id: proposal_id })?;

    // ensure status is up to date
    prop.update_status(&env.block)?;
    let old_status = prop.status;

    let veto_config = prop
        .veto
        .as_ref()
        .ok_or(VetoError::NoVetoConfiguration {})?;

    // Check sender is vetoer
    veto_config.check_is_vetoer(&info)?;

    match prop.status {
        Status::Open => {
            // can only veto an open proposal if veto_before_passed is enabled.
            veto_config.check_veto_before_passed_enabled()?;
        }
        Status::Passed => {
            // if this proposal has veto configured but is in the passed state,
            // the timelock already expired, so provide a more specific error.
            return Err(ContractError::VetoError(VetoError::TimelockExpired {}));
        }
        Status::VetoTimelock { expiration } => {
            // vetoer can veto the proposal iff the timelock is active/not
            // expired. this should never happen since the status updates to
            // passed after the timelock expires, but let's check anyway.
            if expiration.is_expired(&env.block) {
                return Err(ContractError::VetoError(VetoError::TimelockExpired {}));
            }
        }
        // generic status error if the proposal has any other status.
        _ => {
            return Err(ContractError::VetoError(VetoError::InvalidProposalStatus {
                status: prop.status.to_string(),
            }));
        }
    }

    // Update proposal status to vetoed
    prop.status = Status::Vetoed;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    // Add proposal status change hooks
    let proposal_status_changed_hooks = proposal_status_changed_hooks(
        PROPOSAL_HOOKS,
        deps.storage,
        proposal_id,
        old_status.to_string(),
        prop.status.to_string(),
    )?;

    // Add prepropose / deposit module hook which will handle deposit refunds.
    let proposal_creation_policy = CREATION_POLICY.load(deps.storage)?;
    let proposal_completed_hooks =
        proposal_completed_hooks(proposal_creation_policy, proposal_id, prop.status)?;

    Ok(Response::new()
        .add_attribute("action", "veto")
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_submessages(proposal_status_changed_hooks)
        .add_submessages(proposal_completed_hooks))
}

pub fn execute_update_rationale(
    deps: DepsMut,
    info: MessageInfo,
    proposal_id: u64,
    rationale: Option<String>,
) -> Result<Response, ContractError> {
    BALLOTS.update(
        deps.storage,
        // info.sender can't be forged so we implicitly access control
        // with the key.
        (proposal_id, &info.sender),
        |ballot| match ballot {
            Some(ballot) => Ok(Ballot {
                rationale: rationale.clone(),
                ..ballot
            }),
            None => Err(ContractError::NoSuchVote {
                id: proposal_id,
                voter: info.sender.to_string(),
            }),
        },
    )?;

    Ok(Response::default()
        .add_attribute("action", "update_rationale")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("rationale", rationale.as_deref().unwrap_or("_none")))
}

pub fn execute_close(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let mut prop = PROPOSALS.load(deps.storage, proposal_id)?;

    // Update status to ensure that proposals which were open and have
    // expired are moved to "rejected."
    prop.update_status(&env.block)?;
    if prop.status != Status::Rejected {
        return Err(ContractError::WrongCloseStatus {});
    }

    let old_status = prop.status;

    prop.status = Status::Closed;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    // Add proposal status change hooks
    let proposal_status_changed_hooks = proposal_status_changed_hooks(
        PROPOSAL_HOOKS,
        deps.storage,
        proposal_id,
        old_status.to_string(),
        prop.status.to_string(),
    )?;

    // Add prepropose / deposit module hook which will handle deposit refunds.
    let proposal_creation_policy = CREATION_POLICY.load(deps.storage)?;
    let proposal_completed_hooks =
        proposal_completed_hooks(proposal_creation_policy, proposal_id, prop.status)?;

    Ok(Response::default()
        .add_submessages(proposal_status_changed_hooks)
        .add_submessages(proposal_completed_hooks)
        .add_attribute("action", "close")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    threshold: Threshold,
    max_voting_period: Duration,
    min_voting_period: Option<Duration>,
    only_members_execute: bool,
    allow_revoting: bool,
    dao: String,
    close_proposal_on_execution_failure: bool,
    veto: Option<VetoConfig>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Only the DAO may call this method.
    if info.sender != config.dao {
        return Err(ContractError::Unauthorized {});
    }
    threshold.validate()?;
    let dao = deps.api.addr_validate(&dao)?;

    let (min_voting_period, max_voting_period) =
        validate_voting_period(min_voting_period, max_voting_period)?;

    // if veto is configured, validate its fields
    if let Some(veto_config) = &veto {
        veto_config.validate(&deps.as_ref(), &max_voting_period)?;
    };

    CONFIG.save(
        deps.storage,
        &Config {
            threshold,
            max_voting_period,
            min_voting_period,
            only_members_execute,
            allow_revoting,
            dao,
            close_proposal_on_execution_failure,
            veto,
        },
    )?;

    Ok(Response::default()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
}

pub fn execute_update_proposal_creation_policy(
    deps: DepsMut,
    info: MessageInfo,
    new_info: PreProposeInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.dao != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let (initial_policy, messages) = new_info.into_initial_policy_and_messages(config.dao)?;
    CREATION_POLICY.save(deps.storage, &initial_policy)?;

    Ok(Response::default()
        .add_submessages(messages)
        .add_attribute("action", "update_proposal_creation_policy")
        .add_attribute("sender", info.sender)
        .add_attribute("new_policy", format!("{initial_policy:?}")))
}

pub fn add_hook(
    hooks: Hooks,
    storage: &mut dyn Storage,
    validated_address: Addr,
) -> Result<(), ContractError> {
    hooks
        .add_hook(storage, validated_address)
        .map_err(ContractError::HookError)?;
    Ok(())
}

pub fn remove_hook(
    hooks: Hooks,
    storage: &mut dyn Storage,
    validate_address: Addr,
) -> Result<(), ContractError> {
    hooks
        .remove_hook(storage, validate_address)
        .map_err(ContractError::HookError)?;
    Ok(())
}

pub fn execute_add_proposal_hook(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.dao != info.sender {
        // Only DAO can add hooks
        return Err(ContractError::Unauthorized {});
    }

    let validated_address = deps.api.addr_validate(&address)?;

    add_hook(PROPOSAL_HOOKS, deps.storage, validated_address)?;

    Ok(Response::default()
        .add_attribute("action", "add_proposal_hook")
        .add_attribute("address", address))
}

pub fn execute_remove_proposal_hook(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.dao != info.sender {
        // Only DAO can remove hooks
        return Err(ContractError::Unauthorized {});
    }

    let validated_address = deps.api.addr_validate(&address)?;

    remove_hook(PROPOSAL_HOOKS, deps.storage, validated_address)?;

    Ok(Response::default()
        .add_attribute("action", "remove_proposal_hook")
        .add_attribute("address", address))
}

pub fn execute_add_vote_hook(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.dao != info.sender {
        // Only DAO can add hooks
        return Err(ContractError::Unauthorized {});
    }

    let validated_address = deps.api.addr_validate(&address)?;

    add_hook(VOTE_HOOKS, deps.storage, validated_address)?;

    Ok(Response::default()
        .add_attribute("action", "add_vote_hook")
        .add_attribute("address", address))
}

pub fn execute_remove_vote_hook(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.dao != info.sender {
        // Only DAO can remove hooks
        return Err(ContractError::Unauthorized {});
    }

    let validated_address = deps.api.addr_validate(&address)?;

    remove_hook(VOTE_HOOKS, deps.storage, validated_address)?;

    Ok(Response::default()
        .add_attribute("action", "remove_vote_hook")
        .add_attribute("address", address))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => query_config(deps),
        QueryMsg::Dao {} => query_dao(deps),
        QueryMsg::Proposal { proposal_id } => query_proposal(deps, env, proposal_id),
        QueryMsg::ListProposals { start_after, limit } => {
            query_list_proposals(deps, env, start_after, limit)
        }
        QueryMsg::NextProposalId {} => query_next_proposal_id(deps),
        QueryMsg::ProposalCount {} => query_proposal_count(deps),
        QueryMsg::GetVote { proposal_id, voter } => query_vote(deps, proposal_id, voter),
        QueryMsg::ListVotes {
            proposal_id,
            start_after,
            limit,
        } => query_list_votes(deps, proposal_id, start_after, limit),
        QueryMsg::Info {} => query_info(deps),
        QueryMsg::ReverseProposals {
            start_before,
            limit,
        } => query_reverse_proposals(deps, env, start_before, limit),
        QueryMsg::ProposalCreationPolicy {} => query_creation_policy(deps),
        QueryMsg::ProposalHooks {} => to_json_binary(&PROPOSAL_HOOKS.query_hooks(deps)?),
        QueryMsg::VoteHooks {} => to_json_binary(&VOTE_HOOKS.query_hooks(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    to_json_binary(&config)
}

pub fn query_dao(deps: Deps) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    to_json_binary(&config.dao)
}

pub fn query_proposal(deps: Deps, env: Env, id: u64) -> StdResult<Binary> {
    let proposal = PROPOSALS.load(deps.storage, id)?;
    to_json_binary(&proposal.into_response(&env.block, id)?)
}

pub fn query_creation_policy(deps: Deps) -> StdResult<Binary> {
    let policy = CREATION_POLICY.load(deps.storage)?;
    to_json_binary(&policy)
}

pub fn query_list_proposals(
    deps: Deps,
    env: Env,
    start_after: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let min = start_after.map(Bound::exclusive);
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let props: Vec<ProposalResponse> = PROPOSALS
        .range(deps.storage, min, None, cosmwasm_std::Order::Ascending)
        .take(limit as usize)
        .collect::<Result<Vec<(u64, SingleChoiceProposal)>, _>>()?
        .into_iter()
        .map(|(id, proposal)| proposal.into_response(&env.block, id))
        .collect::<StdResult<Vec<ProposalResponse>>>()?;

    to_json_binary(&ProposalListResponse { proposals: props })
}

pub fn query_reverse_proposals(
    deps: Deps,
    env: Env,
    start_before: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let max = start_before.map(Bound::exclusive);
    let props: Vec<ProposalResponse> = PROPOSALS
        .range(deps.storage, None, max, cosmwasm_std::Order::Descending)
        .take(limit as usize)
        .collect::<Result<Vec<(u64, SingleChoiceProposal)>, _>>()?
        .into_iter()
        .map(|(id, proposal)| proposal.into_response(&env.block, id))
        .collect::<StdResult<Vec<ProposalResponse>>>()?;

    to_json_binary(&ProposalListResponse { proposals: props })
}

pub fn query_proposal_count(deps: Deps) -> StdResult<Binary> {
    let proposal_count = PROPOSAL_COUNT.load(deps.storage)?;
    to_json_binary(&proposal_count)
}

pub fn query_next_proposal_id(deps: Deps) -> StdResult<Binary> {
    to_json_binary(&next_proposal_id(deps.storage)?)
}

pub fn query_vote(deps: Deps, proposal_id: u64, voter: String) -> StdResult<Binary> {
    let voter = deps.api.addr_validate(&voter)?;
    let ballot = BALLOTS.may_load(deps.storage, (proposal_id, &voter))?;
    let vote = ballot.map(|ballot| VoteInfo {
        voter,
        vote: ballot.vote,
        power: ballot.power,
        rationale: ballot.rationale,
    });
    to_json_binary(&VoteResponse { vote })
}

pub fn query_list_votes(
    deps: Deps,
    proposal_id: u64,
    start_after: Option<String>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let start_after = start_after
        .map(|addr| deps.api.addr_validate(&addr))
        .transpose()?;
    let min = start_after.as_ref().map(Bound::<&Addr>::exclusive);

    let votes = BALLOTS
        .prefix(proposal_id)
        .range(deps.storage, min, None, Order::Ascending)
        .take(limit as usize)
        .map(|item| {
            let (voter, ballot) = item?;
            Ok(VoteInfo {
                voter,
                vote: ballot.vote,
                power: ballot.power,
                rationale: ballot.rationale,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    to_json_binary(&VoteListResponse { votes })
}

pub fn query_info(deps: Deps) -> StdResult<Binary> {
    let info = cw2::get_contract_version(deps.storage)?;
    to_json_binary(&dao_interface::voting::InfoResponse { info })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let ContractVersion { version, .. } = get_contract_version(deps.storage)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    match msg {
        MigrateMsg::FromV1 {
            close_proposal_on_execution_failure,
            pre_propose_info,
            veto,
        } => {
            // `CONTRACT_VERSION` here is from the data section of the
            // blob we are migrating to. `version` is from storage. If
            // the version in storage matches the version in the blob
            // we are not upgrading.
            if version == CONTRACT_VERSION {
                return Err(ContractError::AlreadyMigrated {});
            }

            let current_config = v1::state::CONFIG.load(deps.storage)?;
            let max_voting_period = v1_duration_to_v2(current_config.max_voting_period);

            // if veto is configured, validate its fields
            if let Some(veto_config) = &veto {
                veto_config.validate(&deps.as_ref(), &max_voting_period)?;
            };

            // Update the stored config to have the new
            // `close_proposal_on_execution_failure` field.
            CONFIG.save(
                deps.storage,
                &Config {
                    threshold: v1_threshold_to_v2(current_config.threshold),
                    max_voting_period,
                    min_voting_period: current_config.min_voting_period.map(v1_duration_to_v2),
                    only_members_execute: current_config.only_members_execute,
                    allow_revoting: current_config.allow_revoting,
                    dao: current_config.dao.clone(),
                    close_proposal_on_execution_failure,
                    veto,
                },
            )?;

            let (initial_policy, pre_propose_messages) =
                pre_propose_info.into_initial_policy_and_messages(current_config.dao)?;
            CREATION_POLICY.save(deps.storage, &initial_policy)?;

            // Update the module's proposals to v2.

            let current_proposals = v1::state::PROPOSALS
                .range(deps.storage, None, None, Order::Ascending)
                .collect::<StdResult<Vec<(u64, v1::proposal::Proposal)>>>()?;

            // Based on gas usage testing, we estimate that we will be
            // able to migrate ~4200 proposals at a time before
            // reaching the block max_gas limit.
            current_proposals
                .into_iter()
                .try_for_each::<_, Result<_, ContractError>>(|(id, prop)| {
                    if prop
                        .deposit_info
                        .map(|info| !info.deposit.is_zero())
                        .unwrap_or(false)
                        && prop.status != voting_v1::Status::Closed
                        && prop.status != voting_v1::Status::Executed
                    {
                        // No migration path for outstanding
                        // deposits.
                        return Err(ContractError::PendingProposals {});
                    }

                    let migrated_proposal = SingleChoiceProposal {
                        title: prop.title,
                        description: prop.description,
                        proposer: prop.proposer,
                        start_height: prop.start_height,
                        min_voting_period: prop.min_voting_period.map(v1_expiration_to_v2),
                        expiration: v1_expiration_to_v2(prop.expiration),
                        threshold: v1_threshold_to_v2(prop.threshold),
                        total_power: prop.total_power,
                        msgs: prop.msgs,
                        status: v1_status_to_v2(prop.status),
                        votes: v1_votes_to_v2(prop.votes),
                        allow_revoting: prop.allow_revoting,
                        veto: None,
                    };

                    PROPOSALS
                        .save(deps.storage, id, &migrated_proposal)
                        .map_err(|e| e.into())
                })?;

            Ok(Response::default()
                .add_attribute("action", "migrate")
                .add_attribute("from", "v1")
                .add_submessages(pre_propose_messages))
        }
        MigrateMsg::FromCompatible {} => Ok(Response::default()
            .add_attribute("action", "migrate")
            .add_attribute("from", "compatible")),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let repl = TaggedReplyId::new(msg.id)?;
    match repl {
        TaggedReplyId::FailedProposalExecution(proposal_id) => {
            PROPOSALS.update(deps.storage, proposal_id, |prop| match prop {
                Some(mut prop) => {
                    prop.status = Status::ExecutionFailed;

                    Ok(prop)
                }
                None => Err(ContractError::NoSuchProposal { id: proposal_id }),
            })?;

            Ok(Response::new()
                .add_attribute("proposal_execution_failed", proposal_id.to_string())
                .add_attribute("error", msg.result.into_result().err().unwrap_or_default()))
        }
        TaggedReplyId::FailedProposalHook(idx) => {
            let addr = PROPOSAL_HOOKS.remove_hook_by_index(deps.storage, idx)?;
            Ok(Response::new().add_attribute("removed_proposal_hook", format!("{addr}:{idx}")))
        }
        TaggedReplyId::FailedVoteHook(idx) => {
            let addr = VOTE_HOOKS.remove_hook_by_index(deps.storage, idx)?;
            Ok(Response::new().add_attribute("removed_vote_hook", format!("{addr}:{idx}")))
        }
        TaggedReplyId::PreProposeModuleInstantiation => {
            let res = parse_reply_instantiate_data(msg)?;

            let module = deps.api.addr_validate(&res.contract_address)?;
            CREATION_POLICY.save(
                deps.storage,
                &ProposalCreationPolicy::Module { addr: module },
            )?;

            // per the cosmwasm docs, we shouldn't have to forward
            // data like this, yet here we are and it does not work if
            // we do not.
            //
            // <https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#handling-the-reply>
            match res.data {
                Some(data) => Ok(Response::new()
                    .add_attribute("update_pre_propose_module", res.contract_address)
                    .set_data(data)),
                None => Ok(Response::new()
                    .add_attribute("update_pre_propose_module", res.contract_address)),
            }
        }
        TaggedReplyId::FailedPreProposeModuleHook => {
            let addr = match CREATION_POLICY.load(deps.storage)? {
                ProposalCreationPolicy::Anyone {} => {
                    // Something is off if we're getting this
                    // reply and we don't have a pre-propose
                    // module installed. This should be
                    // unreachable.
                    return Err(ContractError::InvalidReplyID {
                        id: failed_pre_propose_module_hook_id(),
                    });
                }
                ProposalCreationPolicy::Module { addr } => {
                    // If we are here, our pre-propose module has
                    // errored while receiving a proposal
                    // hook. Rest in peace pre-propose module.
                    CREATION_POLICY.save(deps.storage, &ProposalCreationPolicy::Anyone {})?;
                    addr
                }
            };
            Ok(Response::new().add_attribute("failed_prepropose_hook", format!("{addr}")))
        }
    }
}
