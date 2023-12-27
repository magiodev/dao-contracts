#!/bin/bash
# set -x
# Set Osmosis Testnet variables
export CHAIN_ID="osmosis"
export HOME_OSMOSIS=${HOME}/.osmosis
export TESTNET_NAME="osmosis-testnet"
export DENOM="uosmo"
export BECH32_HRP="osmo"
export CONFIG_DIR=".osmosis"
export BINARY="osmosisd"
export RPC="tcp://localhost:26679"
export TXFLAG_1="--chain-id ${CHAIN_ID} --gas-prices 0.025uosmo --gas auto --gas-adjustment 1.3 --broadcast-mode block"
export TXFLAG_2="--chain-id ${CHAIN_ID} --gas 25000000 --fees 300000uosmo --broadcast-mode block --no-admin"
export TXFLAG_3="--chain-id ${CHAIN_ID} --gas 25000000 --fees 300000uosmo --keyring-backend test --home ${HOME_OSMOSIS} --broadcast-mode block"
export TXFLAG_4="--chain-id ${CHAIN_ID} --gas 25000000 --fees 300000uosmo --keyring-backend test --home ${HOME_OSMOSIS} --broadcast-mode block --no-admin"

export NODE="tcp://localhost:26679"

# Set the contract specific variables

# Deploy wasm codes locally on your locally running osmosis chain

## STORE CODE CW CORE
RES=$(${BINARY} tx wasm store ../../artifacts/dao_dao_core.wasm  --node ${NODE} --from alice  ${TXFLAG_3} --output json -y | jq)
#echo ${RES}
export CODE_ID_CW_CORE=$(echo $RES | jq -r '.logs[0].events[-1].attributes[1].value')
echo "CODE_ID_CW_CORE = $CODE_ID_CW_CORE"

## STORE CODE PROP SINGLE
RES=$(${BINARY} tx wasm store ../../artifacts/dao_proposal_single.wasm  --node ${NODE} --from alice  ${TXFLAG_3} --output json -y | jq)
#echo ${RES}
export CODE_ID_PROP_SINGLE=$(echo $RES | jq -r '.logs[0].events[-1].attributes[1].value')
echo "$CODE_ID_PROP_SINGLE = $CODE_ID_PROP_SINGLE"

## STORE CODE PROP SINGLE INSTANT
RES=$(${BINARY} tx wasm store ../../artifacts/dao_proposal_single_instant.wasm  --node ${NODE} --from alice  ${TXFLAG_3} --output json -y | jq)
#echo ${RES}
export CODE_ID_PROP_SINGLE_INSTANT=$(echo $RES | jq -r '.logs[0].events[-1].attributes[1].value')
echo "$CODE_ID_PROP_SINGLE_INSTANT = $CODE_ID_PROP_SINGLE_INSTANT"


## STORE CODE VOTING MODULE
RES=$(${BINARY} tx wasm store ../../artifacts/dao_voting_cw4.wasm  --node ${NODE} --from alice  ${TXFLAG_3} --output json -y | jq )
#echo ${RES}
export CODE_ID_VOTING=$(echo $RES | jq -r '.logs[0].events[-1].attributes[1].value')
echo "CODE_ID_VOTING =  $CODE_ID_VOTING"

## STORE CW ADMIN MODULE
RES=$(${BINARY} tx wasm store ../../artifacts/cw_admin_factory.wasm  --node ${NODE} --from alice  ${TXFLAG_3} --output json -y | jq )
#echo ${RES}
export CODE_ID_CW_ADMIN_FACTORY=$(echo $RES | jq -r '.logs[0].events[-1].attributes[1].value')
echo "CODE_ID_CW_ADMIN_FACTORY =  $CODE_ID_CW_ADMIN_FACTORY"


## STORE CW ADMIN MODULE
RES=$(${BINARY} tx wasm store ../../artifacts/cw4_group.wasm  --node ${NODE} --from alice  ${TXFLAG_3} --output json -y | jq )
#echo ${RES}
export CODE_ID_CW4_GROUP=$(echo $RES | jq -r '.logs[0].events[-1].attributes[1].value')
echo "CODE_ID_CW4_GROUP =  ${CODE_ID_CW4_GROUP}"


#export CODE_ID_CW_CORE=4897
export CW_CORE_LABEL="qsr_rebalance_dao"

#export CODE_ID_PROP_SINGLE=4902  # Example code ID for Proposal Module
#export CODE_ID_PROP_SINGLE_INSTANT=5761  # Example code ID for Proposal Module
export LABEL_PROP_SINGLE="qsr_dao_proposal_single"
export LABEL_PROP_SINGLE_INSTANT="qsr_dao_proposal_single_instant"
#export CODE_ID_VOTING=4903  # Example code ID for Voting Module
export LABEL_VOTING="qsr_dao_voting"
export ADMIN_FACTORY_CONTRACT_ADDR="osmo1v5k3527dt2vt67848h8jk0az9dyl8sunsqaapznf2j9tm4arxxfs7gwa0n"  # Contract address variable
export ADMIN=null  # Admin address



###############################################################
# # DAO-PROPOSAL-SINGLE MESSAGE PREP
###############################################################
PROP_MODULE_MSG_1='{
        "allow_revoting": false,
        "max_voting_period": {
          "time": 3600
        },
        "close_proposal_on_execution_failure": true,
        "pre_propose_info": {"anyone_may_propose":{}},
        "only_members_execute": true,
        "threshold": {
          "threshold_quorum": {
            "quorum": {
              "percent": "0.20"
            },
            "threshold": {
              "majority": {}
            }
          }
        }
      }'

echo $PROP_MODULE_MSG_1 | jq > "prop_message_1.json"
ENCODED_PROP_MESSAGE_1=$(echo -n $PROP_MODULE_MSG_1 | tr -d '[:space:]' | openssl base64 | tr -d '[:space:]')
echo $ENCODED_PROP_MESSAGE_1 > "encoded_prop_message_1.txt"
echo -e "\nENCODED PROP MESSAGE 1:\n$ENCODED_PROP_MESSAGE_1"


###############################################################
# # DAO-PROPOSAL-SINGLE-INSTANT MESSAGE PREP
###############################################################
PROP_MODULE_MSG_2='{
        "allow_revoting": false,
        "max_voting_period": {
          "height": 1
        },
        "close_proposal_on_execution_failure": true,
        "pre_propose_info": {
          "anyone_may_propose":{}
        },
        "only_members_execute": true,
        "threshold": {
          "absolute_count": {
              "threshold": "2"
          }
        }
      }'

echo $PROP_MODULE_MSG_2 | jq > "prop_message_2.json"
ENCODED_PROP_MESSAGE_2=$(echo -n $PROP_MODULE_MSG_2 | tr -d '[:space:]' | openssl base64 | tr -d '[:space:]')
echo $ENCODED_PROP_MESSAGE_2 > "encoded_prop_message_2.txt"
echo -e "\nENCODED PROP MESSAGE 2:\n$ENCODED_PROP_MESSAGE_2"


###############################################################
# VOTING MODULE MESSAGE PREPARATION
###############################################################
# TODO - Need to change the addresses here as per our use case for testings.
# This initial members are based on osmosis local net accounts ( alice, bob..)
VOTING_MSG='{
  "group_contract": {
    "new": {
      "cw4_group_code_id": '"${CODE_ID_CW4_GROUP}"',
      "initial_members": [
        {
          "addr": "osmo1t8eh66t2w5k67kwurmn5gqhtq6d2ja0vp7jmmq",
          "weight": 0
        },
        {
          "addr": "osmo1ez43ye5qn3q2zwh8uvswppvducwnkq6wjqc87d",
          "weight": 1
        },
        {
          "addr": "osmo1zaavvzxez0elundtn32qnk9lkm8kmcsz2tlhe7",
          "weight": 1
        },
        {
          "addr": "osmo185fflsvwrz0cx46w6qada7mdy92m6kx4qm4l9k",
          "weight": 1
        },
        {
          "addr": "osmo194580p9pyxakf3y3nqqk9hc3w9a7x0yrnv7wcz",
          "weight": 1
        }
      ]
    }
  }
}'

echo $VOTING_MSG | jq > "voting_msg.json"
ENCODED_VOTING_MESSAGE=$(echo $VOTING_MSG | tr -d '[:space:]' | openssl base64 | tr -d '[:space:]')
echo $ENCODED_VOTING_MESSAGE > "encoded_voting_message.txt"
echo -e "\nENCODED VOTING MESSAGE:\n$ENCODED_VOTING_MESSAGE"

###############################################################
# CW CORE INIT MESSAGE PREPARATION
###############################################################
CW_CORE_INIT='{
  "admin": '${ADMIN}',
  "automatically_add_cw20s": true,
  "automatically_add_cw721s": true,
  "description": "QSR Rebalance DAO",
  "name": "QSR Rebalance DAO",
  "proposal_modules_instantiate_info": [
    {
      "admin": {
        "core_module": {}
      },
      "code_id": '${CODE_ID_PROP_SINGLE}',
      "label": "'${LABEL_PROP_SINGLE}'",
      "msg": "'${ENCODED_PROP_MESSAGE_1}'",
       "funds": []
    },
    {
      "admin": {
          "core_module": {}
      },
      "code_id": '${CODE_ID_PROP_SINGLE_INSTANT}',
      "label": "'${LABEL_PROP_SINGLE_INSTANT}'",
      "msg": "'${ENCODED_PROP_MESSAGE_2}'",
      "funds": []
      }
  ],
  "voting_module_instantiate_info": {
    "admin": {
      "core_module": {}
    },
    "code_id": '${CODE_ID_VOTING}',
    "label": "'${LABEL_VOTING}'",
    "msg": "'${ENCODED_VOTING_MESSAGE}'",
     "funds": []
  }
}'
echo "CW_CORE_INIT = $CW_CORE_INIT"
echo $CW_CORE_INIT | jq > "cw_core_init.json"
CW_CORE_STRIPPED=$(echo -n $CW_CORE_INIT | tr -d '[:space:]')
echo -e "CW-CORE INSTANTIATE MESSAGE:\n$CW_CORE_STRIPPED"
CW_CORE_ENCODED=$(echo -n $CW_CORE_STRIPPED | openssl base64 | tr -d '[:space:]')
echo $CW_CORE_ENCODED > "cw_core_encoded.txt"
echo -e "\nCW-CORE ENCODED MESSAGE:\n$CW_CORE_ENCODED"

# init with factory
INIT_MSG="{\"instantiate_contract_with_self_admin\":{\"code_id\":${CODE_ID_CW_CORE}, \"label\": \"${CW_CORE_LABEL}\", \"instantiate_msg\":\"$CW_CORE_ENCODED\"}}"
echo $INIT_MSG | jq > "cw_core_init_with_factory.json"
echo -e "INIT MESSAGE:\n$INIT_MSG"

#####################################################
# Not running the command from the script fo now due to keying not known for the user.

# instantiate with factory
# echo 'instantiating cw-core with factory'
# NOTE - Commented the command part to do it manually
# ${BINARY} tx wasm execute $ADMIN_FACTORY_CONTRACT_ADDR "$INIT_MSG" --from $KEY_NAME --node $NODE $TXFLAG_2

echo 'instantiating cw-core without factory'
# NOTE - Commented the command part to do it manually
RES=$(${BINARY} tx wasm instantiate $CODE_ID_CW_CORE "$CW_CORE_INIT" --label "${CW_CORE_LABEL}" --from alice --node $NODE $TXFLAG_4 --output json | jq)

echo "INSTANTIATE RESULT - ${RES}"
DAO_DAO_CONTRACT_ADDRESS=$(${BINARY} query wasm list-contract-by-code $CODE_ID_CW_CORE --output json --node $NODE | jq -r '.contracts[0]')
echo "DAO DAO CONTRACT - ${DAO_DAO_CONTRACT_ADDRESS}"
