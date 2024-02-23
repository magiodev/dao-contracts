use crate::proposal::SingleChoiceProposal;
use cosmwasm_schema::{
    cw_serde,
    serde::{self, Deserialize, Deserializer, Serializer},
};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_hooks::Hooks;
use cw_storage_plus::{Item, Map};
use cw_utils::Duration;
use dao_voting::{
    pre_propose::ProposalCreationPolicy, threshold::Threshold, veto::VetoConfig, voting::Vote,
};

#[cw_serde]
pub enum RangeExecuteMsg {
    /// Submit a range to the range middleware
    SubmitNewRange { new_range: NewRange },
}

#[cw_serde]
pub struct NewRange {
    pub cl_vault_address: String,
    pub lower_price: Decimal,
    pub upper_price: Decimal,
}

/// A vote cast for an instant proposal containing message_hash and message_signature.
#[cw_serde]
pub struct VoteSignature {
    /// Message hash
    #[serde(serialize_with = "as_base64", deserialize_with = "from_base64")]
    pub message_hash: Vec<u8>,
    /// Signature of message hash
    #[serde(serialize_with = "as_base64", deserialize_with = "from_base64")]
    pub signature: Vec<u8>,
    /// Public key that signed message hash
    #[serde(serialize_with = "as_base64", deserialize_with = "from_base64")]
    pub public_key: Vec<u8>,
}

fn as_base64<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&base64::encode(bytes))
}

fn from_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    base64::decode(&s).map_err(serde::de::Error::custom)
}

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
    /// The default maximum amount of time a proposal may be voted on
    /// before expiring.
    pub max_voting_period: Duration,
    /// The minimum amount of time a proposal must be open before
    /// passing. A proposal may fail before this amount of time has
    /// elapsed, but it will not pass. This can be useful for
    /// preventing governance attacks wherein an attacker aquires a
    /// large number of tokens and forces a proposal through.
    pub min_voting_period: Option<Duration>,
    /// If set to true only members may execute passed
    /// proposals. Otherwise, any address may execute a passed
    /// proposal.
    pub only_members_execute: bool,
    /// Allows changing votes before the proposal expires. If this is
    /// enabled proposals will not be able to complete early as final
    /// vote information is not known until the time of proposal
    /// expiration.
    pub allow_revoting: bool,
    /// The address of the DAO that this governance module is
    /// associated with.
    pub dao: Addr,
    /// If set to true proposals will be closed if their execution
    /// fails. Otherwise, proposals will remain open after execution
    /// failure. For example, with this enabled a proposal to send 5
    /// tokens out of a DAO's treasury with 4 tokens would be closed when
    /// it is executed. With this disabled, that same proposal would
    /// remain open until the DAO's treasury was large enough for it to be
    /// executed.
    pub close_proposal_on_execution_failure: bool,
    /// Optional veto configuration. If set to `None`, veto option
    /// is disabled. Otherwise contains the configuration for veto flow.
    pub veto: Option<VetoConfig>,
}

/// The current top level config for the module.  The "config" key was
/// previously used to store configs for v1 DAOs.
pub const CONFIG: Item<Config> = Item::new("config_v2");
/// The number of proposals that have been created.
pub const PROPOSAL_COUNT: Item<u64> = Item::new("proposal_count");
pub const PROPOSALS: Map<u64, SingleChoiceProposal> = Map::new("proposals_v2");
pub const BALLOTS: Map<(u64, &Addr), Ballot> = Map::new("ballots");
/// Consumers of proposal state change hooks.
pub const PROPOSAL_HOOKS: Hooks = Hooks::new("proposal_hooks");
/// Consumers of vote hooks.
pub const VOTE_HOOKS: Hooks = Hooks::new("vote_hooks");
/// The address of the pre-propose module associated with this
/// proposal module (if any).
pub const CREATION_POLICY: Item<ProposalCreationPolicy> = Item::new("creation_policy");
