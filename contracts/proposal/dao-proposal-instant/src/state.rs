use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_hooks::Hooks;
use cw_storage_plus::{Item, Map};
use cw_utils::Duration;
use dao_voting::{pre_propose::ProposalCreationPolicy, threshold::Threshold, voting::Vote};

use crate::proposal::SingleChoiceInstantProposal;

/// A vote cast for a proposal.
#[cw_serde]
pub struct Ballot {
    /// The amount of voting power behind the vote.
    pub power: Uint128,
    /// The position.
    pub vote: Vote,

    /// An optional rationale for why this vote was cast. If the key
    /// is missing (i.e. the ballot was cast in a v1 proposal module),
    /// we deserialize into None (i.e. Option::default()).
    #[serde(default)]
    pub rationale: Option<String>,
}
/// The governance module's configuration.
#[cw_serde]
pub struct Config {
    /// The threshold a proposal must reach to complete.
    pub threshold: Threshold,
    /// If set to true only members may execute passed
    /// proposals. Otherwise, any address may execute a passed
    /// proposal.
    pub only_members_execute: bool,
    /// The address of the DAO that this governance module is
    /// associated with.
    pub dao: Addr,
}

/// The current top level config for the module.  The "config" key was
/// previously used to store configs for v1 DAOs.
pub const CONFIG: Item<Config> = Item::new("config_v2");
/// The number of proposals that have been created.
pub const PROPOSAL_COUNT: Item<u64> = Item::new("proposal_count");
pub const PROPOSALS: Map<u64, SingleChoiceInstantProposal> = Map::new("proposals_v2");
pub const BALLOTS: Map<(u64, &Addr), Ballot> = Map::new("ballots");
/// Consumers of proposal state change hooks.
pub const PROPOSAL_HOOKS: Hooks = Hooks::new("proposal_hooks");
/// Consumers of vote hooks.
pub const VOTE_HOOKS: Hooks = Hooks::new("vote_hooks");
/// The address of the pre-propose module associated with this
/// proposal module (if any).
pub const CREATION_POLICY: Item<ProposalCreationPolicy> = Item::new("creation_policy");
