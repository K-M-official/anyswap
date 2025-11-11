use anchor_lang::prelude::*;

pub mod instructions;
pub mod state;
pub  mod error;

use instructions::*;
declare_id!("DLysqmqcrGm9nkaRfyq3Ys1FJsDDnGYwAdoZxBGKQBJt");

#[program]
pub mod fucksol2 {

    use super::*;

    /// 创建新的交易对
    pub fn create_pair(
        ctx: Context<CreatePair>,
        token_0: Pubkey,
        token_1: Pubkey,
    ) -> Result<()> {
        create_pair::create_pair(ctx, token_0, token_1)
    }

    /// 添加流动性
    pub fn add_liquidity(
        ctx: Context<AddLiquidity>,
        amount_0: u64,
        amount_1: u64,
    ) -> Result<()> {
        add_liquidity::add_liquidity(ctx, amount_0, amount_1)
    }

    /// 移除流动性
    pub fn remove_liquidity(
        ctx: Context<RemoveLiquidity>,
        lp_amount: u64,
    ) -> Result<()> {
        remove_liquidity::remove_liquidity(ctx, lp_amount)
    }

    /// 交换代币
    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
    ) -> Result<()> {
        swap::swap(ctx, amount_in, min_amount_out)
    }
}
