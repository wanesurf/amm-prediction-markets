use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    // Increment {},
    // Reset {
    //     count: i32,
    // },
    CreateMarket {
        description: String,
        initial_liquidity: Uint128,
    },
    AddLiquidity {
        market_id: u64,
        amount: Uint128,
    },
    RemoveLiquidity {
        market_id: u64,
        amount: Uint128,
    },
    BuyShares {
        market_id: u64,
        outcome: String,
        amount: Uint128,
    },
    SellShares {
        market_id: u64,
        outcome: String,
        amount: Uint128,
    },
    ResolveMarket {
        market_id: u64,
        winning_outcome: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(GetCountResponse)]
    GetCount {},
}

// We define a custom struct for each query response
#[cw_serde]
pub struct GetCountResponse {
    pub count: i32,
}
