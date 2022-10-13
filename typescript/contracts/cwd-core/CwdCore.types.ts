/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.19.0.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export type Admin = {
  address: {
    addr: string;
  };
} | {
  core_module: {};
};
export type Binary = string;
export interface InstantiateMsg {
  admin?: string | null;
  automatically_add_cw20s: boolean;
  automatically_add_cw721s: boolean;
  dao_uri?: string | null;
  description: string;
  image_url?: string | null;
  initial_items?: InitialItem[] | null;
  name: string;
  proposal_modules_instantiate_info: ModuleInstantiateInfo[];
  voting_module_instantiate_info: ModuleInstantiateInfo;
}
export interface InitialItem {
  key: string;
  value: string;
}
export interface ModuleInstantiateInfo {
  admin?: Admin | null;
  code_id: number;
  label: string;
  msg: Binary;
}
export type ExecuteMsg = {
  execute_admin_msgs: {
    msgs: CosmosMsgForEmpty[];
  };
} | {
  execute_proposal_hook: {
    msgs: CosmosMsgForEmpty[];
  };
} | {
  pause: {
    duration: Duration;
  };
} | {
  receive: Cw20ReceiveMsg;
} | {
  receive_nft: Cw721ReceiveMsg;
} | {
  remove_item: {
    key: string;
  };
} | {
  set_item: {
    addr: string;
    key: string;
  };
} | {
  nominate_admin: {
    admin?: string | null;
  };
} | {
  accept_admin_nomination: {};
} | {
  withdraw_admin_nomination: {};
} | {
  update_config: {
    config: Config;
  };
} | {
  update_cw20_list: {
    to_add: string[];
    to_remove: string[];
  };
} | {
  update_cw721_list: {
    to_add: string[];
    to_remove: string[];
  };
} | {
  update_proposal_modules: {
    to_add: ModuleInstantiateInfo[];
    to_disable: string[];
  };
} | {
  update_voting_module: {
    module: ModuleInstantiateInfo;
  };
} | {
  update_sub_daos: {
    to_add: SubDao[];
    to_remove: string[];
  };
};
export type CosmosMsgForEmpty = {
  bank: BankMsg;
} | {
  custom: Empty;
} | {
  staking: StakingMsg;
} | {
  distribution: DistributionMsg;
} | {
  stargate: {
    type_url: string;
    value: Binary;
    [k: string]: unknown;
  };
} | {
  ibc: IbcMsg;
} | {
  wasm: WasmMsg;
} | {
  gov: GovMsg;
};
export type BankMsg = {
  send: {
    amount: Coin[];
    to_address: string;
    [k: string]: unknown;
  };
} | {
  burn: {
    amount: Coin[];
    [k: string]: unknown;
  };
};
export type Uint128 = string;
export type StakingMsg = {
  delegate: {
    amount: Coin;
    validator: string;
    [k: string]: unknown;
  };
} | {
  undelegate: {
    amount: Coin;
    validator: string;
    [k: string]: unknown;
  };
} | {
  redelegate: {
    amount: Coin;
    dst_validator: string;
    src_validator: string;
    [k: string]: unknown;
  };
};
export type DistributionMsg = {
  set_withdraw_address: {
    address: string;
    [k: string]: unknown;
  };
} | {
  withdraw_delegator_reward: {
    validator: string;
    [k: string]: unknown;
  };
};
export type IbcMsg = {
  transfer: {
    amount: Coin;
    channel_id: string;
    timeout: IbcTimeout;
    to_address: string;
    [k: string]: unknown;
  };
} | {
  send_packet: {
    channel_id: string;
    data: Binary;
    timeout: IbcTimeout;
    [k: string]: unknown;
  };
} | {
  close_channel: {
    channel_id: string;
    [k: string]: unknown;
  };
};
export type Timestamp = Uint64;
export type Uint64 = string;
export type WasmMsg = {
  execute: {
    contract_addr: string;
    funds: Coin[];
    msg: Binary;
    [k: string]: unknown;
  };
} | {
  instantiate: {
    admin?: string | null;
    code_id: number;
    funds: Coin[];
    label: string;
    msg: Binary;
    [k: string]: unknown;
  };
} | {
  migrate: {
    contract_addr: string;
    msg: Binary;
    new_code_id: number;
    [k: string]: unknown;
  };
} | {
  update_admin: {
    admin: string;
    contract_addr: string;
    [k: string]: unknown;
  };
} | {
  clear_admin: {
    contract_addr: string;
    [k: string]: unknown;
  };
};
export type GovMsg = {
  vote: {
    proposal_id: number;
    vote: VoteOption;
    [k: string]: unknown;
  };
};
export type VoteOption = "yes" | "no" | "abstain" | "no_with_veto";
export type Duration = {
  height: number;
} | {
  time: number;
};
export interface Coin {
  amount: Uint128;
  denom: string;
  [k: string]: unknown;
}
export interface Empty {
  [k: string]: unknown;
}
export interface IbcTimeout {
  block?: IbcTimeoutBlock | null;
  timestamp?: Timestamp | null;
  [k: string]: unknown;
}
export interface IbcTimeoutBlock {
  height: number;
  revision: number;
  [k: string]: unknown;
}
export interface Cw20ReceiveMsg {
  amount: Uint128;
  msg: Binary;
  sender: string;
  [k: string]: unknown;
}
export interface Cw721ReceiveMsg {
  msg: Binary;
  sender: string;
  token_id: string;
  [k: string]: unknown;
}
export interface Config {
  automatically_add_cw20s: boolean;
  automatically_add_cw721s: boolean;
  dao_uri?: string | null;
  description: string;
  image_url?: string | null;
  name: string;
}
export interface SubDao {
  addr: string;
  charter?: string | null;
}
export type QueryMsg = {
  admin: {};
} | {
  admin_nomination: {};
} | {
  config: {};
} | {
  cw20_balances: {
    limit?: number | null;
    start_after?: string | null;
  };
} | {
  cw20_token_list: {
    limit?: number | null;
    start_after?: string | null;
  };
} | {
  cw721_token_list: {
    limit?: number | null;
    start_after?: string | null;
  };
} | {
  dump_state: {};
} | {
  get_item: {
    key: string;
  };
} | {
  list_items: {
    limit?: number | null;
    start_after?: string | null;
  };
} | {
  proposal_modules: {
    limit?: number | null;
    start_after?: string | null;
  };
} | {
  active_proposal_modules: {
    limit?: number | null;
    start_after?: string | null;
  };
} | {
  pause_info: {};
} | {
  voting_module: {};
} | {
  list_sub_daos: {
    limit?: number | null;
    start_after?: string | null;
  };
} | {
  dao_u_r_i: {};
} | {
  voting_power_at_height: {
    address: string;
    height?: number | null;
  };
} | {
  total_power_at_height: {
    height?: number | null;
  };
} | {
  info: {};
};
export type MigrateMsg = {
  from_v1: {
    dao_uri?: string | null;
  };
} | {
  from_compatible: {};
};
export type Addr = string;
export type ProposalModuleStatus = "enabled" | "disabled";
export type ArrayOfProposalModule = ProposalModule[];
export interface ProposalModule {
  address: Addr;
  prefix: string;
  status: ProposalModuleStatus;
}
export interface AdminNominationResponse {
  nomination?: Addr | null;
}
export interface Cw20BalanceResponse {
  addr: Addr;
  balance: Uint128;
}
export type ArrayOfAddr = Addr[];
export interface DaoURIResponse {
  dao_uri?: string | null;
}
export type PauseInfoResponse = {
  paused: {
    expiration: Expiration;
  };
} | {
  unpaused: {};
};
export type Expiration = {
  at_height: number;
} | {
  at_time: Timestamp;
} | {
  never: {
    [k: string]: unknown;
  };
};
export interface DumpStateResponse {
  active_proposal_module_count: number;
  admin: Addr;
  config: Config;
  pause_info: PauseInfoResponse;
  proposal_modules: ProposalModule[];
  total_proposal_module_count: number;
  version: ContractVersion;
  voting_module: Addr;
}
export interface ContractVersion {
  contract: string;
  version: string;
}
export interface GetItemResponse {
  item?: string | null;
}
export interface InfoResponse {
  info: ContractVersion;
}
export type ArrayOfString = string[];
export type ArrayOfSubDao = SubDao[];
export interface TotalPowerAtHeightResponse {
  height: number;
  power: Uint128;
}
export interface VotingPowerAtHeightResponse {
  height: number;
  power: Uint128;
}