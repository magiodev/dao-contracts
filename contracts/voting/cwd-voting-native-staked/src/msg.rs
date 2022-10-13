use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use cw_controllers::ClaimsResponse;
use cw_utils::Duration;
use cwd_interface::voting::InfoResponse;
use cwd_interface::voting::TotalPowerAtHeightResponse;
use cwd_interface::voting::VotingPowerAtHeightResponse;
use cwd_interface::Admin;
use cwd_macros::{info_query, voting_query};

use crate::state::Config;

#[cw_serde]
pub struct InstantiateMsg {
    // Owner can update all configs including changing the owner. This will generally be a DAO.
    pub owner: Option<Admin>,
    // Manager can update all configs except changing the owner. This will generally be an operations multisig for a DAO.
    pub manager: Option<String>,
    // Token denom e.g. ujuno, or some ibc denom
    pub denom: String,
    // How long until the tokens become liquid again
    pub unstaking_duration: Option<Duration>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Stake {},
    Unstake {
        amount: Uint128,
    },
    UpdateConfig {
        owner: Option<String>,
        manager: Option<String>,
        duration: Option<Duration>,
    },
    Claim {},
}

#[voting_query]
#[info_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Addr)]
    Dao {},
    #[returns(Config)]
    GetConfig {},
    #[returns(ClaimsResponse)]
    Claims { address: String },
    #[returns(ListStakersResponse)]
    ListStakers {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct ListStakersResponse {
    pub stakers: Vec<StakerBalanceResponse>,
}

#[cw_serde]
pub struct StakerBalanceResponse {
    pub address: String,
    pub balance: Uint128,
}
