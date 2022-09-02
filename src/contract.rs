#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Uint128, Binary, Coin, Addr, Deps, DepsMut, Env, MessageInfo, Response, StdResult, BankMsg};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, GetBalanceResponse, GetOwnerResponse, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE, Balance, BALANCES};


// version info for migration info
const CONTRACT_NAME: &str = "crates.io:funds-split";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const TOKEN_NAME: &str = "usei";

const FEE_PERCENT: Uint128 = Uint128::new(2);
const BASIS_POINT: Uint128 = Uint128::new(100);

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Split { user1, user2 } => try_split(deps, info, user1, user2),
        ExecuteMsg::Withdraw { amount } => try_withdraw(deps, info, amount),
    }
}

pub fn try_split(deps: DepsMut, info: MessageInfo, user1: String, user2: String) -> Result<Response, ContractError> {

    let state = STATE.load(deps.storage);

    let owner_balance = BALANCES.may_load(deps.storage, &state.as_ref().unwrap().owner)?;

    let total_funds = info.funds[0].amount * (BASIS_POINT - FEE_PERCENT )/ BASIS_POINT;

    let fees = info.funds[0].amount - total_funds;

    let owner_balance = match owner_balance {
        Some(one) => Balance {
            balance : Coin { amount: one.balance.amount + fees, denom: TOKEN_NAME.to_string() },
        },
        None => Balance {
            balance : Coin { amount: fees, denom: TOKEN_NAME.to_string() },
        } 
    };

    BALANCES.save(deps.storage, &state.unwrap().owner, &owner_balance);

    let user_balance_1 = BALANCES.may_load(deps.storage, &deps.api.addr_validate(&user1)?)?;
    
    if info.funds.len() > 1 || info.funds[0].denom != TOKEN_NAME.to_string() {
        return Err(ContractError::InvalidTokenTransfer{});
    }

    let funds1 = total_funds / Uint128::new(2);
    let funds2 = total_funds - funds1;
    let user_balance_1 = match user_balance_1 {
        Some(one) => Balance {
            balance : Coin { amount: one.balance.amount + funds1, denom: TOKEN_NAME.to_string() },
        },
        None => Balance {
            balance : Coin { amount: funds1, denom: TOKEN_NAME.to_string() },
        } 
    };

    BALANCES.save(deps.storage, &deps.api.addr_validate(&user1)?, &user_balance_1);

    let user_balance_2 = BALANCES.may_load(deps.storage, &deps.api.addr_validate(&user2)?)?;

    let user_balance_2 = match user_balance_2 {
        Some(one) => Balance {
            balance: Coin { amount: one.balance.amount + funds2, denom: TOKEN_NAME.to_string() },
        },
        None => Balance {
            balance: Coin { amount: funds2, denom: TOKEN_NAME.to_string() }
        } 
    };

    BALANCES.save(deps.storage, &deps.api.addr_validate(&user2)?, &user_balance_2);

    Ok(Response::new().add_attribute("method","try_split"))
}


pub fn try_withdraw(deps: DepsMut, info: MessageInfo, amount: Option<Coin>) -> Result<Response, ContractError> {

    let user_balance = BALANCES.may_load(deps.storage, &info.sender)?;

    if amount != None && amount.as_ref().unwrap().denom != TOKEN_NAME.to_string() {
        return Err(ContractError::InvalidTokenTransfer{});
    }

    if user_balance == None {
        return Err(ContractError::Unauthorized{});
    }

    let user_balance = user_balance.unwrap();
    let init_amount = user_balance.balance.amount;
    if amount != None && amount.as_ref().unwrap().amount > user_balance.balance.amount {
        return Err(ContractError::InvalidAmountError{});
    }


    let withdrawable_amount = if let Some(amount) = amount {
        amount
    }else {
        user_balance.balance
    };

    let user_balance = Balance {
        balance: Coin { amount: init_amount - withdrawable_amount.amount, denom: TOKEN_NAME.to_string() }
    };

    if user_balance.balance.amount > Uint128::new(0) {
        BALANCES.save(deps.storage, &info.sender, &user_balance);
    }else{
        BALANCES.remove(deps.storage, &info.sender);
    }
    Ok(send_tokens(info.sender, withdrawable_amount, "withdraw"))

}

// this is a helper to move the tokens, so the business logic is easy to read
fn send_tokens(to_address: Addr, amount: Coin, action: &str) -> Response {

    let mut coin_vec = Vec::new();
    coin_vec.push(amount);
    Response::new()
        .add_message(BankMsg::Send {
            to_address: to_address.clone().into(),
            amount: coin_vec,
        })
        .add_attribute("action", action)
        .add_attribute("to", to_address)
}



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetBalance { user } => to_binary(&query_balance(deps, user)?),
        QueryMsg::GetOwner {} => to_binary(&query_owner(deps)?),
    }
}

fn query_balance(deps: Deps, user: String) -> StdResult<GetBalanceResponse> {
    let balances = BALANCES.may_load(deps.storage, &deps.api.addr_validate(&user)?)?;

    if balances == None {
        return Ok(GetBalanceResponse { balance: Coin{ amount: Uint128::new(0), denom: TOKEN_NAME.to_string() } });
    }

    Ok(GetBalanceResponse { balance: balances.unwrap().balance })
}

fn query_owner(deps: Deps) -> StdResult<GetOwnerResponse> {
    let state = STATE.load(deps.storage);

    Ok(GetOwnerResponse { owner: state.unwrap().owner })
}


#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn read_owner() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetOwner {}).unwrap();
        let value: GetOwnerResponse = from_binary(&res).unwrap();
        assert_eq!(Addr::unchecked("creator"), value.owner);
    }

    #[test]
    fn split() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { };
        let info = mock_info("creator", &coins(1000, "usei"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let bob_wallet = Addr::unchecked("bob");
        let alice_wallet = Addr::unchecked("alice");
        let msg = ExecuteMsg::Split {
            user1: bob_wallet.clone().into(),
            user2: alice_wallet.clone().into(),
        };
        let info = mock_info("creator", &coins(1000, "usei"));

        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(0, _res.messages.len());
    }

    #[test]
    fn read_balances() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { };
        let info = mock_info("creator", &coins(1000, "usei"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let bob_wallet = Addr::unchecked("bob");
        let alice_wallet = Addr::unchecked("alice");
        let msg = ExecuteMsg::Split {
            user1: bob_wallet.clone().into(),
            user2: alice_wallet.clone().into(),
        };
        let info = mock_info("creator", &coins(1000, "usei"));

        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();


        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetBalance { user: bob_wallet.clone().into() }).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(1000)*(BASIS_POINT - FEE_PERCENT)/(Uint128::new(2) * BASIS_POINT), value.balance.amount)        
    }



    #[test]
    fn other_token_not_allowed() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { };
        let info = mock_info("creator", &coins(1000, "usei"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let bob_wallet = Addr::unchecked("bob");
        let alice_wallet = Addr::unchecked("alice");
        let msg = ExecuteMsg::Split {
            user1: bob_wallet.clone().into(),
            user2: alice_wallet.clone().into(),
        };
        let info = mock_info("creator", &coins(1000, "BTC"));

        assert!(execute(deps.as_mut(), mock_env(), info, msg).is_err());

    }

    #[test]
    fn withdraw_balance_token_amount() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { };
        let info = mock_info("creator", &coins(1000, "usei"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let bob_wallet = Addr::unchecked("bob");
        let alice_wallet = Addr::unchecked("alice");
        let msg = ExecuteMsg::Split {
            user1: bob_wallet.clone().into(),
            user2: alice_wallet.clone().into(),
        };
        let info = mock_info("creator", &coins(1000, "usei"));

        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg :: Withdraw {
            amount: Some(Coin{
                amount: Uint128::new(400),
                denom: "usei".to_string()
            })
        };        

        let info = mock_info("bob", &coins(1000, "usei"));

        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &coins(1000, "usei"));

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetBalance { user: bob_wallet.clone().into() }).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        let bob_balance = (Uint128::new(1000)*(BASIS_POINT - FEE_PERCENT)/(Uint128::new(2)*BASIS_POINT)) - Uint128::new(400);
        assert_eq!(bob_balance, value.balance.amount)        

    }

    #[test]
    fn withdraw_balance_token_full_amount() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { };
        let info = mock_info("creator", &coins(1000, "usei"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let bob_wallet = Addr::unchecked("bob");
        let alice_wallet = Addr::unchecked("alice");
        let msg = ExecuteMsg::Split {
            user1: bob_wallet.clone().into(),
            user2: alice_wallet.clone().into(),
        };
        let info = mock_info("creator", &coins(1000, "usei"));

        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg :: Withdraw {
            amount: None
        };        

        let info = mock_info("bob", &coins(1000, "usei"));

        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &coins(1000, "usei"));

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetBalance { user: bob_wallet.clone().into() }).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(0), value.balance.amount)        

    }

    #[test]
    fn accumulate_owner_fees() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { };
        let info = mock_info("creator", &coins(1000, "usei"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let bob_wallet = Addr::unchecked("bob");
        let alice_wallet = Addr::unchecked("alice");
        let owner_wallet = Addr::unchecked("creator");
        let msg = ExecuteMsg::Split {
            user1: bob_wallet.clone().into(),
            user2: alice_wallet.clone().into(),
        };
        let info = mock_info("creator", &coins(1000, "usei"));

        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let fees = Uint128::new(1000) * FEE_PERCENT / BASIS_POINT;

        let info = mock_info("creator", &coins(1000, "usei"));

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetBalance { user: owner_wallet.clone().into() }).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(fees, value.balance.amount)           

    }


    #[test]
    fn collect_owner_fees() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { };
        let info = mock_info("creator", &coins(1000, "usei"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let bob_wallet = Addr::unchecked("bob");
        let alice_wallet = Addr::unchecked("alice");
        let owner_wallet = Addr::unchecked("creator");
        let msg = ExecuteMsg::Split {
            user1: bob_wallet.clone().into(),
            user2: alice_wallet.clone().into(),
        };
        let info = mock_info("creator", &coins(1000, "usei"));

        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &coins(1000, "usei"));
        
        let msg = ExecuteMsg :: Withdraw {
            amount: None
        };        

        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("creator", &coins(1000, "usei"));

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetBalance { user: owner_wallet.clone().into() }).unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(0), value.balance.amount)           

    }    

}
