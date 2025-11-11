use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};
use crate::state::Pair;

#[derive(Accounts)]
#[instruction(token_0: Pubkey, token_1: Pubkey)]
pub struct CreatePair<'info> {
    #[account(
        init,
        payer = authority,
        space = Pair::LEN,
        seeds = [
            b"pair",
            token_0.as_ref(),
            token_1.as_ref(),
        ],
        bump
    )]
    pub pair: Account<'info, Pair>,
    
    /// LP Token Mint - 由 pair PDA 作为 mint authority
    #[account(
        init,
        payer = authority,
        mint::decimals = 9,
        mint::authority = pair,
        seeds = [
            b"lp_mint",
            token_0.as_ref(),
            token_1.as_ref(),
        ],
        bump
    )]
    pub lp_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn create_pair(
    ctx: Context<CreatePair>,
    token_0: Pubkey,
    token_1: Pubkey,
) -> Result<()> {
    let pair = &mut ctx.accounts.pair;
    pair.token_0 = token_0;
    pair.token_1 = token_1;
    pair.lp_mint = ctx.accounts.lp_mint.key();
    pair.reserve_0 = 0;
    pair.reserve_1 = 0;
    pair.bump = ctx.bumps.pair;
    
    msg!("交易对已创建: {} / {}, LP Mint: {}", token_0, token_1, pair.lp_mint);
    Ok(())
}
