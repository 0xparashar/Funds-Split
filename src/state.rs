use cosmwasm_std::{Addr, Coin};
use cw_storage_plus::Item;
use cw_storage_plus::Map;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub owner: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Balance {
    pub balance: Coin,
}

pub const STATE: Item<State> = Item::new("state");

pub const BALANCES: Map<&Addr, Balance> = Map::new("balances");
