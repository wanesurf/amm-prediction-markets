use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub count: i32,
    pub owner: Addr,
}

pub const STATE: Item<State> = Item::new("state");

//storages
pub const MARKETS: Map<u64, Market> = Map::new("markets");
pub const MARKET_COUNT: Item<u64> = Item::new("market_count");
pub const BUYERS: Map<(u64, Addr), Buyer> = Map::new("buyers");
pub const LIQUIDITY_PROVIDERS: Map<(u64, Addr), LiquidityProvider> =
    Map::new("liquidity_providers");

// State Structures
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Market {
    pub creator: Addr,
    pub description: String,
    pub shares_yes: Uint128,
    pub shares_no: Uint128,
    pub total_liquidity: Uint128,
    pub total_liquidity_shares: Uint128,
    pub resolved: bool,
    pub winning_outcome: Option<String>,
    pub price_yes: Uint128,
    pub price_no: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Buyer {
    pub address: Addr,
    pub shares_yes: Uint128,
    pub shares_no: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct LiquidityProvider {
    pub address: Addr,
    pub contributed_liquidity: Uint128,
}
