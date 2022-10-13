/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.19.0.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export type Duration = {
  height: number;
} | {
  time: number;
};
export interface InstantiateMsg {
  manager?: string | null;
  owner?: string | null;
  token_address: string;
  unstaking_duration?: Duration | null;
}
export type ExecuteMsg = {
  receive: Cw20ReceiveMsg;
} | {
  unstake: {
    amount: Uint128;
  };
} | {
  claim: {};
} | {
  update_config: {
    duration?: Duration | null;
    manager?: string | null;
    owner?: string | null;
  };
} | {
  add_hook: {
    addr: string;
  };
} | {
  remove_hook: {
    addr: string;
  };
};
export type Uint128 = string;
export type Binary = string;
export interface Cw20ReceiveMsg {
  amount: Uint128;
  msg: Binary;
  sender: string;
  [k: string]: unknown;
}
export type QueryMsg = {
  staked_balance_at_height: {
    address: string;
    height?: number | null;
  };
} | {
  total_staked_at_height: {
    height?: number | null;
  };
} | {
  staked_value: {
    address: string;
  };
} | {
  total_value: {};
} | {
  get_config: {};
} | {
  claims: {
    address: string;
  };
} | {
  get_hooks: {};
} | {
  list_stakers: {
    limit?: number | null;
    start_after?: string | null;
  };
};
export type MigrateMsg = {
  from_beta: {
    manager?: string | null;
  };
} | {
  from_compatible: {};
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
export type Timestamp = Uint64;
export type Uint64 = string;
export interface ClaimsResponse {
  claims: Claim[];
  [k: string]: unknown;
}
export interface Claim {
  amount: Uint128;
  release_at: Expiration;
  [k: string]: unknown;
}
export type Addr = string;
export interface Config {
  manager?: Addr | null;
  owner?: Addr | null;
  token_address: Addr;
  unstaking_duration?: Duration | null;
}
export interface GetHooksResponse {
  hooks: string[];
}
export interface ListStakersResponse {
  stakers: StakerBalanceResponse[];
}
export interface StakerBalanceResponse {
  address: string;
  balance: Uint128;
}
export interface StakedBalanceAtHeightResponse {
  balance: Uint128;
  height: number;
}
export interface StakedValueResponse {
  value: Uint128;
}
export interface TotalStakedAtHeightResponse {
  height: number;
  total: Uint128;
}
export interface TotalValueResponse {
  total: Uint128;
}