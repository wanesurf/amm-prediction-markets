#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, GetCountResponse, InstantiateMsg, QueryMsg};
use crate::state::{
    Buyer, LiquidityProvider, Market, BUYERS, LIQUIDITY_PROVIDERS, MARKETS, MARKET_COUNT, STATE,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:truth-markets-contracts";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
// Constants
const FEE_PERCENTAGE: u128 = 2; // 2% trading fee
const DECIMAL_PRECISION: u128 = 1_000_000_00; // For fractional calculations

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
    // .add_attribute("count", msg.count.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateMarket {
            description,
            initial_liquidity,
        } => execute::create_market(deps, info, description, initial_liquidity),
        ExecuteMsg::AddLiquidity { market_id, amount } => {
            execute::add_liquidity(deps, info, market_id, amount)
        }
        ExecuteMsg::RemoveLiquidity { market_id, amount } => {
            execute::remove_liquidity(deps, info, market_id, amount)
        }
        ExecuteMsg::BuyShares {
            market_id,
            outcome,
            amount,
        } => execute::buy_shares(deps, info, market_id, outcome, amount),
        ExecuteMsg::SellShares {
            market_id,
            outcome,
            amount,
        } => execute::sell_shares(deps, info, market_id, outcome, amount),
        ExecuteMsg::ResolveMarket {
            market_id,
            winning_outcome,
        } => execute::resolve_market(deps, info, market_id, winning_outcome),
    }
}

pub mod execute {
    use super::*;

    /// Create a new prediction market
    pub fn create_market(
        deps: DepsMut,
        info: MessageInfo,
        description: String,
        initial_liquidity: Uint128,
    ) -> Result<Response, ContractError> {
        let market_id = MARKET_COUNT.may_load(deps.storage)?.unwrap_or(0) + 1;

        if initial_liquidity.is_zero() {
            return Err(ContractError::Unauthorized {});
        }

        let price_yes = initial_liquidity / (initial_liquidity + initial_liquidity);
        let price_no = initial_liquidity / (initial_liquidity + initial_liquidity);

        let shares_yes = initial_liquidity;
        let shares_no = initial_liquidity;

        let total_liquidity_shares = shares_yes * shares_no;

        //    let new_total_liquidity = new_shares_yes * new_shares_no;
        //     market.total_liquidity = new_total_liquidity.pow(1 / 2);

        let market = Market {
            creator: info.sender.clone(),
            description,
            shares_yes: initial_liquidity,
            shares_no: initial_liquidity,
            total_liquidity: initial_liquidity,
            resolved: false,
            winning_outcome: None,
            price_yes: price_yes,
            price_no: price_no,
            total_liquidity_shares: total_liquidity_shares,
        };

        MARKETS.save(deps.storage, market_id, &market)?;
        MARKET_COUNT.save(deps.storage, &market_id)?;

        Ok(Response::new()
            .add_attribute("action", "create_market")
            .add_attribute("market_id", market_id.to_string()))
    }
    /// Add liquidity to an existing market
    pub fn add_liquidity(
        deps: DepsMut,
        info: MessageInfo,
        market_id: u64,
        amount: Uint128,
    ) -> Result<Response, ContractError> {
        //When a Liquidity Provider adds liquidity to a market, they in fact increase the number of shares in all pools in that market.
        let mut market = MARKETS.load(deps.storage, market_id)?;

        if market.resolved {
            return Err(ContractError::Std(StdError::generic_err(
                "Cannot add liquidity to a resolved market",
            )));
        }

        if amount.is_zero() {
            return Err(ContractError::Std(StdError::generic_err(
                "Amount must be greater than zero",
            )));
        }

        // Calculate the invariant before adding liquidity
        // let invariant = market.shares_yes * market.shares_no;

        // Temporarily add liquidity to both outcome pools
        let temp_shares_yes = market.shares_yes + amount;
        let temp_shares_no = market.shares_no + amount;

        //If the number of outcome shares is equal (i.e. if the outcome prices are equal), adding liquidity will not change the balance of the equation, and therefore the Liquidity Provider will only receive shares of the Liquidity Pool in return for adding liquidity to the market.
        //If, on the contrary, the number of shares in each pool is unbalanced (i.e. if the outcome prices are not equal), then adding liquidity would change the balance of the equation, which would cause a change in outcome prices.

        if market.shares_yes != market.shares_no {
            // Rebalance the pools by giving shares of the asset with the higher price to the Liquidity Provider
            let (new_shares_yes, new_shares_no, shares_yes_to_provider, shares_no_to_provider) =
                if market.shares_yes < market.shares_no {
                    // Rebalance the pools to maintain the invariant and keep prices constant
                    let new_shares_yes = (temp_shares_no * market.price_no) / market.price_yes;
                    let new_shares_no = temp_shares_no;
                    let shares_yes_to_provider = temp_shares_yes - new_shares_yes;
                    let shares_no_to_provider = Uint128::zero();
                    (
                        new_shares_yes,
                        new_shares_no,
                        shares_yes_to_provider,
                        shares_no_to_provider,
                    )
                } else {
                    let new_shares_yes = temp_shares_yes;
                    let new_shares_no = (temp_shares_yes * market.price_yes) / market.price_no;
                    let shares_yes_to_provider = Uint128::zero();
                    let shares_no_to_provider = temp_shares_no - new_shares_no;
                    (
                        new_shares_yes,
                        new_shares_no,
                        shares_yes_to_provider,
                        shares_no_to_provider,
                    )
                };

            // Update the market state
            market.shares_yes = new_shares_yes;
            market.shares_no = new_shares_no;
            market.total_liquidity += amount;

            // Update liquidity provider's contribution
            LIQUIDITY_PROVIDERS.update(
                deps.storage,
                (market_id, info.sender.clone()),
                |record| -> StdResult<LiquidityProvider> {
                    let mut provider = record.unwrap_or(LiquidityProvider {
                        address: info.sender.clone(),
                        contributed_liquidity: Uint128::zero(),
                    });
                    provider.contributed_liquidity += amount;
                    Ok(provider)
                },
            )?;

            // Give the liquidity provider their shares
            BUYERS.update(
                deps.storage,
                (market_id, info.sender.clone()),
                |record| -> StdResult<Buyer> {
                    let mut buyer = record.unwrap_or(Buyer {
                        address: info.sender.clone(),
                        shares_yes: Uint128::zero(),
                        shares_no: Uint128::zero(),
                    });
                    buyer.shares_yes += shares_yes_to_provider;
                    buyer.shares_no += shares_no_to_provider;
                    Ok(buyer)
                },
            )?;

            // market.price_yes = calculate_price(new_shares_yes, new_shares_no);
            // market.price_no = calculate_price(new_shares_no, new_shares_yes);
        } else {
            // If the invariant is not broken, simply add liquidity to both pools
            market.shares_yes += amount;
            market.shares_no += amount;
            market.total_liquidity += amount;

            // Update liquidity provider's contribution
            LIQUIDITY_PROVIDERS.update(
                deps.storage,
                (market_id, info.sender.clone()),
                |record| -> StdResult<LiquidityProvider> {
                    let mut provider = record.unwrap_or(LiquidityProvider {
                        address: info.sender.clone(),
                        contributed_liquidity: Uint128::zero(),
                    });
                    provider.contributed_liquidity += amount;
                    Ok(provider)
                },
            )?;
        }

        // Update market prices
        market.price_yes = calculate_price(market.shares_yes, market.shares_no);
        market.price_no = calculate_price(market.shares_no, market.shares_yes);

        MARKETS.save(deps.storage, market_id, &market)?;

        Ok(Response::new()
            .add_attribute("action", "add_liquidity")
            .add_attribute("market_id", market_id.to_string())
            .add_attribute("liquidity_added", amount.to_string()))
    }

    /// Remove liquidity from an existing market
    pub fn remove_liquidity(
        deps: DepsMut,
        info: MessageInfo,
        market_id: u64,
        amount: Uint128,
    ) -> Result<Response, ContractError> {
        let mut market = MARKETS.load(deps.storage, market_id)?;

        if market.resolved {
            return Err(ContractError::Std(StdError::generic_err(
                "Cannot remove liquidity from a resolved market",
            )));
        }

        if amount.is_zero() {
            return Err(ContractError::Std(StdError::generic_err(
                "Amount must be greater than zero",
            )));
        }

        // Check if the liquidity provider has enough liquidity to remove
        let mut provider = LIQUIDITY_PROVIDERS
            .load(deps.storage, (market_id, info.sender.clone()))
            .map_err(|_| StdError::generic_err("Liquidity provider not found"))?;

        if provider.contributed_liquidity < amount {
            return Err(ContractError::Std(StdError::generic_err(
                "Insufficient liquidity to remove",
            )));
        }

        // Calculate the share of the liquidity pool the provider owns
        let liquidity_share = amount * market.total_liquidity / provider.contributed_liquidity;

        // Calculate the shares to withdraw from each outcome pool
        let shares_yes_to_withdraw = liquidity_share * market.shares_yes / market.total_liquidity;
        let shares_no_to_withdraw = liquidity_share * market.shares_no / market.total_liquidity;

        // Update the market state
        market.shares_yes -= shares_yes_to_withdraw;
        market.shares_no -= shares_no_to_withdraw;
        market.total_liquidity -= amount;

        // Update the liquidity provider's contribution
        provider.contributed_liquidity -= amount;
        LIQUIDITY_PROVIDERS.save(deps.storage, (market_id, info.sender.clone()), &provider)?;

        // Update market prices
        market.price_yes = calculate_price(market.shares_yes, market.shares_no);
        market.price_no = calculate_price(market.shares_no, market.shares_yes);

        MARKETS.save(deps.storage, market_id, &market)?;

        // Send the withdrawn funds to the provider (pseudo-code, replace with actual token transfer logic)
        // deps.querier.send_tokens(&info.sender, amount)?;

        Ok(Response::new()
            .add_attribute("action", "remove_liquidity")
            .add_attribute("market_id", market_id.to_string())
            .add_attribute("liquidity_removed", amount.to_string())
            .add_attribute("shares_yes_withdrawn", shares_yes_to_withdraw.to_string())
            .add_attribute("shares_no_withdrawn", shares_no_to_withdraw.to_string())
            .add_attribute("price_yes", market.price_yes.to_string())
            .add_attribute("price_no", market.price_no.to_string()))
    }

    /// Buy shares (at the current price)
    pub fn buy_shares(
        deps: DepsMut,
        info: MessageInfo,
        market_id: u64,
        outcome: String,
        amount: Uint128,
    ) -> Result<Response, ContractError> {
        let mut market = MARKETS.load(deps.storage, market_id)?;

        if market.resolved {
            return Err(ContractError::Std(StdError::generic_err(
                "Cannot trade in a resolved market",
            )));
        }

        // Apply trading fee
        let fee = amount * Uint128::from(FEE_PERCENTAGE as u128) / Uint128::from(100u128);
        let net_amount: Uint128 = amount;

        //TODO let's see what we do with the fee?

        // Add fee to the market's total liquidity (to be distributed later)
        //   let net_amount = amount - fee;
        // market.total_liquidity += fee;

        // Constant product invariant
        // let invariant = market.shares_yes * market.shares_no;

        let (shares_bought, new_price_yes, new_price_no) = match outcome.as_str() {
            "YES" => {
                let new_shares_no = market.shares_no + net_amount;
                let new_shares_yes = market.shares_yes + net_amount;

                let new_shares_yes_balanced = market.total_liquidity.pow(2) / (new_shares_no);

                let shares_bought = new_shares_yes - new_shares_yes_balanced;

                market.shares_yes = new_shares_yes_balanced;
                market.shares_no = new_shares_no;

                // market.shares_yes = Uint128::zero();
                // market.shares_no = Uint128::zero();

                let new_price_yes = calculate_price(new_shares_yes_balanced, new_shares_no);
                let new_price_no = calculate_price(new_shares_no, new_shares_yes_balanced);

                market.price_yes = new_price_yes;
                market.price_no = new_price_no;

                // Update buyer's shares
                BUYERS.update(
                    deps.storage,
                    (market_id, info.sender.clone()),
                    |record| -> StdResult<Buyer> {
                        let mut buyer = record.unwrap_or(Buyer {
                            address: info.sender.clone(),
                            shares_yes: Uint128::zero(),
                            shares_no: Uint128::zero(),
                        });
                        if outcome == "YES" {
                            buyer.shares_yes += shares_bought;
                        } else {
                            buyer.shares_no += shares_bought;
                        }
                        Ok(buyer)
                    },
                )?;

                (shares_bought, new_price_yes, new_price_no)
            }
            "NO" => {
                let new_shares_yes = market.shares_yes + net_amount;
                let new_shares_no = market.shares_no + net_amount;

                let new_shares_no_balanced = market.total_liquidity.pow(2) / (new_shares_yes);

                let shares_bought = new_shares_no - new_shares_no_balanced;

                market.shares_yes = new_shares_yes;
                market.shares_no = new_shares_no_balanced;

                let new_price_yes = calculate_price(new_shares_yes, new_shares_no_balanced);
                let new_price_no = calculate_price(new_shares_no_balanced, new_shares_yes);

                market.price_yes = new_price_yes;
                market.price_no = new_price_no;

                // Update buyer's shares
                BUYERS.update(
                    deps.storage,
                    (market_id, info.sender.clone()),
                    |record| -> StdResult<Buyer> {
                        let mut buyer = record.unwrap_or(Buyer {
                            address: info.sender.clone(),
                            shares_yes: Uint128::zero(),
                            shares_no: Uint128::zero(),
                        });
                        if outcome == "YES" {
                            buyer.shares_yes += shares_bought;
                        } else {
                            buyer.shares_no += shares_bought;
                        }
                        Ok(buyer)
                    },
                )?;

                (shares_bought, new_price_yes, new_price_no)
            }

            _ => return Err(ContractError::Std(StdError::generic_err("Invalid outcome"))),
        };

        MARKETS.save(deps.storage, market_id, &market)?;

        Ok(Response::new()
            .add_attribute("action", "buy_shares")
            .add_attribute("market_id", market_id.to_string())
            .add_attribute("outcome", outcome)
            .add_attribute("shares_bought", shares_bought.to_string())
            .add_attribute("price_yes", new_price_yes.to_string())
            .add_attribute("price_no", new_price_no.to_string()))
    }

    pub fn sell_shares(
        deps: DepsMut,
        info: MessageInfo,
        market_id: u64,
        outcome: String,
        amount: Uint128,
    ) -> Result<Response, ContractError> {
        let mut market = MARKETS.load(deps.storage, market_id)?;

        if market.resolved {
            return Err(ContractError::Std(StdError::generic_err(
                "Cannot trade in a resolved market",
            )));
        }

        // Check if the user has enough shares to sell
        let mut buyer = BUYERS
            .load(deps.storage, (market_id, info.sender.clone()))
            .map_err(|_| StdError::generic_err("Buyer not found"))?;

        let shares_to_sell = match outcome.as_str() {
            "YES" => {
                if buyer.shares_yes < amount {
                    return Err(ContractError::Std(StdError::generic_err(
                        "Insufficient YES shares to sell",
                    )));
                }
                buyer.shares_yes -= amount;
                amount
            }
            "NO" => {
                if buyer.shares_no < amount {
                    return Err(ContractError::Std(StdError::generic_err(
                        "Insufficient NO shares to sell",
                    )));
                }
                buyer.shares_no -= amount;
                amount
            }
            _ => return Err(ContractError::Std(StdError::generic_err("Invalid outcome"))),
        };

        // Constant product invariant
        let invariant = market.shares_yes * market.shares_no;

        let (usdc_received, new_price_yes, new_price_no) = match outcome.as_str() {
            "YES" => {
                let new_shares_yes = market.shares_yes - shares_to_sell;
                let new_shares_no = invariant / new_shares_yes;
                let usdc_received = market.shares_no - new_shares_no;

                market.shares_yes = new_shares_yes;
                market.shares_no = new_shares_no;

                let new_price_yes = calculate_price(market.shares_yes, market.shares_no);
                let new_price_no = calculate_price(market.shares_no, market.shares_yes);

                (usdc_received, new_price_yes, new_price_no)
            }
            "NO" => {
                let new_shares_no = market.shares_no - shares_to_sell;
                let new_shares_yes = invariant / new_shares_no;
                let usdc_received = market.shares_yes - new_shares_yes;

                market.shares_yes = new_shares_yes;
                market.shares_no = new_shares_no;

                let new_price_yes = calculate_price(market.shares_yes, market.shares_no);
                let new_price_no = calculate_price(market.shares_no, market.shares_yes);

                (usdc_received, new_price_yes, new_price_no)
            }
            _ => return Err(ContractError::Std(StdError::generic_err("Invalid outcome"))),
        };

        // Update buyer's shares
        BUYERS.save(deps.storage, (market_id, info.sender.clone()), &buyer)?;

        // Update market prices
        market.price_yes = new_price_yes;
        market.price_no = new_price_no;

        MARKETS.save(deps.storage, market_id, &market)?;

        // Send USDC to the seller (pseudo-code, replace with actual token transfer logic)
        // deps.querier.send_tokens(&info.sender, usdc_received)?;

        Ok(Response::new()
            .add_attribute("action", "sell_shares")
            .add_attribute("market_id", market_id.to_string())
            .add_attribute("outcome", outcome)
            .add_attribute("shares_sold", shares_to_sell.to_string())
            .add_attribute("usdc_received", usdc_received.to_string())
            .add_attribute("price_yes", new_price_yes.to_string())
            .add_attribute("price_no", new_price_no.to_string()))
    }

    /// Resolve a market and distribute payouts
    pub fn resolve_market(
        deps: DepsMut,
        info: MessageInfo,
        market_id: u64,
        winning_outcome: String,
    ) -> Result<Response, ContractError> {
        let mut market = MARKETS.load(deps.storage, market_id)?;

        if market.resolved {
            return Err(ContractError::Std(StdError::generic_err(
                "Market is already resolved",
            )));
        }

        if info.sender != market.creator {
            return Err(ContractError::Std(StdError::generic_err(
                "Only the market creator can resolve the market",
            )));
        }

        market.resolved = true;
        market.winning_outcome = Some(winning_outcome.clone());

        // Distribute payouts to buyers
        let buyers = BUYERS
            .prefix(market_id)
            .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
            .collect::<StdResult<Vec<_>>>()?;

        for (buyer_addr, buyer) in buyers {
            let payout = if winning_outcome == "YES" {
                buyer.shares_yes
            } else {
                buyer.shares_no
            };

            // Send payout to the buyer (pseudo-code, replace with actual token transfer logic)
            // deps.querier.send_tokens(&buyer_addr, payout)?;
        }

        // Distribute fees to liquidity providers
        let liquidity_providers = LIQUIDITY_PROVIDERS
            .prefix(market_id)
            .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
            .collect::<StdResult<Vec<_>>>()?;

        for (provider_addr, provider) in liquidity_providers {
            // Send payout to the provider (pseudo-code, replace with actual token transfer logic)
            // deps.querier.send_tokens(&provider_addr, provider.contributed_liquidity)?;
        }

        Ok(Response::new()
            .add_attribute("action", "resolve_market")
            .add_attribute("market_id", market_id.to_string())
            .add_attribute("winning_outcome", winning_outcome))
    }

    fn calculate_price(share_pool: Uint128, other_pool: Uint128) -> Uint128 {
        let total_shares = share_pool + other_pool;
        if total_shares.is_zero() {
            return Uint128::zero();
        }
        other_pool * Uint128::from(DECIMAL_PRECISION) / total_shares
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_json_binary(&query::count(deps)?),
    }
}

pub mod query {
    use super::*;

    pub fn count(deps: Deps) -> StdResult<GetCountResponse> {
        let state = STATE.load(deps.storage)?;
        Ok(GetCountResponse { count: state.count })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_json};

    // Helper function to initialize a market
    fn setup_market(deps: DepsMut) -> u64 {
        let market_id = 1;
        let creator = Addr::unchecked("creator");
        let initial_liquidity = Uint128::new(1000);

        // Create a market
        let msg = ExecuteMsg::CreateMarket {
            description: "Will it rain tomorrow?".to_string(),
            initial_liquidity,
        };
        let info = mock_info(creator.as_str(), &coins(1000, "USDC"));
        execute(deps, mock_env(), info, msg).unwrap();

        market_id
    }

    #[test]
    fn test_add_liquidity_to_unresolved_market() {
        let mut deps = mock_dependencies();
        let market_id = setup_market(deps.as_mut());

        // Add liquidity to the market
        let liquidity_provider = Addr::unchecked("provider");
        let amount = Uint128::new(500);
        let msg = ExecuteMsg::AddLiquidity { market_id, amount };
        let info = mock_info(liquidity_provider.as_str(), &coins(500, "USDC"));
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Verify the response attributes
        assert_eq!(
            res.attributes,
            vec![
                ("action", "add_liquidity"),
                ("market_id", "1"),
                ("liquidity_added", "500"),
            ]
        );

        // Verify the updated market state
        let market: Market = MARKETS.load(&deps.storage, market_id).unwrap();
        assert_eq!(market.shares_yes, Uint128::new(1500)); // 1000 + 500
        assert_eq!(market.shares_no, Uint128::new(1500)); // 1000 + 500
        assert_eq!(market.total_liquidity, Uint128::new(1500)); // 1000 + 500
        assert_eq!(market.price_yes, Uint128::from(50000000u128)); // 1500 / (1500 + 1500) * DECIMAL_PRECISION
        assert_eq!(market.price_no, Uint128::from(50000000u128)); // 1500 / (1500 + 1500) * DECIMAL_PRECISION

        // Verify the liquidity provider's contribution
        let provider: LiquidityProvider = LIQUIDITY_PROVIDERS
            .load(&deps.storage, (market_id, liquidity_provider.clone()))
            .unwrap();
        assert_eq!(provider.contributed_liquidity, Uint128::new(500));
    }

    #[test]
    fn test_add_liquidity_with_zero_amount() {
        let mut deps = mock_dependencies();
        let market_id = setup_market(deps.as_mut());

        // Attempt to add zero liquidity
        let liquidity_provider = Addr::unchecked("provider");
        let amount = Uint128::zero();
        let msg = ExecuteMsg::AddLiquidity { market_id, amount };
        let info = mock_info(liquidity_provider.as_str(), &coins(0, "USDC"));
        let res = execute(deps.as_mut(), mock_env(), info, msg);

        // Verify that the operation fails
        assert_eq!(
            res.unwrap_err(),
            ContractError::Std(StdError::generic_err("Amount must be greater than zero"))
        );
    }

    #[test]
    fn test_buy_shares() {
        let mut deps = mock_dependencies();
        let market_id = setup_market(deps.as_mut());

        // Simulate trades to create unequal prices
        let trader = Addr::unchecked("trader");
        let trade_amount = Uint128::new(300);
        let msg = ExecuteMsg::BuyShares {
            market_id,
            outcome: "YES".to_string(),
            amount: trade_amount,
        };
        let info = mock_info(trader.as_str(), &coins(300, "USDC"));
        // Liquidity Value: 1300 USDC
        // Outcomes YES share : 769.2307692308
        // Outcomes NO share : 1300
        // Outcomes YES price : 0.6283228613 =  (1300/(1300+769))
        // Outcomes NO price : 0.3716771387 =  (769/(1300+769))
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let market: Market = MARKETS.load(&deps.storage, market_id).unwrap();

        assert_eq!(market.shares_yes, Uint128::new(769));
        assert_eq!(market.shares_no, Uint128::new(1300));
        assert_eq!(market.price_yes, Uint128::from(62832286u128));
        assert_eq!(market.price_no, Uint128::from(37167713u128));
        // TODO: Verify the buyer's shares
        // Bob shares YES: 1294 - 769 = 531
        let buyer: Buyer = BUYERS
            .load(&deps.storage, (market_id, trader.clone()))
            .unwrap();
        assert_eq!(buyer.shares_yes, Uint128::new(531));
        assert_eq!(buyer.shares_no, Uint128::new(0));
        // Bob shares NO: 0
    }

    #[test]
    fn test_add_liquidity_with_unequal_prices() {
        let mut deps = mock_dependencies();
        let market_id = setup_market(deps.as_mut());

        // Simulate trades to create unequal prices
        let trader = Addr::unchecked("trader");
        let trade_amount = Uint128::new(300);
        let msg = ExecuteMsg::BuyShares {
            market_id,
            outcome: "YES".to_string(),
            amount: trade_amount,
        };
        let info = mock_info(trader.as_str(), &coins(300, "USDC"));
        // Liquidity Value: 1300 USDC
        // Outcomes YES share : 769.2307692308
        // Outcomes NO share : 1300
        // Outcomes YES price : 0.6282527881 =  (1300/(1300+769.2307692308))
        // Outcomes NO price : 0.3717472119 =  (769.2307692308/(1300+769.2307692308))
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Add liquidity to the market with unequal prices
        let liquidity_provider = Addr::unchecked("provider");
        let amount = Uint128::new(1000);
        let msg = ExecuteMsg::AddLiquidity { market_id, amount };
        let info = mock_info(liquidity_provider.as_str(), &coins(1000, "USDC"));
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Verify the response attributes
        assert_eq!(
            res.attributes,
            vec![
                ("action", "add_liquidity"),
                ("market_id", "1"),
                ("liquidity_added", "1000"),
            ]
        );

        // Verify the updated market state
        let market: Market = MARKETS.load(&deps.storage, market_id).unwrap();
        assert_eq!(market.shares_yes, Uint128::new(1360)); // Rebalanced shares = (Shares_no * price_no)/price_yes = ((1300+1000) * 0.3717472119) /0.6282527881 = 1,360.90 //TODO: round up???
        assert_eq!(market.shares_no, Uint128::new(2300)); // Rebalanced share
        assert_eq!(market.total_liquidity, Uint128::new(2000)); // 1000 + 500
        assert_eq!(market.price_yes, Uint128::from(62832286u128)); // Recalculated price (same as before)
        assert_eq!(market.price_no, Uint128::from(37167713u128)); // Recalculated price (same as before)

        // Verify the liquidity provider's contribution
        let provider: LiquidityProvider = LIQUIDITY_PROVIDERS
            .load(&deps.storage, (market_id, liquidity_provider.clone()))
            .unwrap();
        assert_eq!(provider.contributed_liquidity, Uint128::new(500));
    }
}
