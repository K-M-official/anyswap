use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, Transfer};
use crate::state::Pair;
use crate::error::ErrorCode;

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    
    #[account(
        mut,
        address = pair.lp_mint @ ErrorCode::InvalidLpMint
    )]
    pub lp_mint: Account<'info, Mint>,
    
    /// 用户的 LP 代币账户（销毁 LP）
    #[account(mut)]
    pub lp_token_account: Box<Account<'info, TokenAccount>>,
    
    /// 交易对的 token_0 账户（转出代币给用户）
    #[account(mut)]
    pub pair_token_0_account: Box<Account<'info, TokenAccount>>,
    
    /// 交易对的 token_1 账户（转出代币给用户）
    #[account(mut)]
    pub pair_token_1_account: Box<Account<'info, TokenAccount>>,
    
    /// 用户的 token_0 账户（接收返还的代币）
    #[account(mut)]
    pub user_token_0_account: Box<Account<'info, TokenAccount>>,
    
    /// 用户的 token_1 账户（接收返还的代币）
    #[account(mut)]
    pub user_token_1_account: Box<Account<'info, TokenAccount>>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

/// 移除流动性
/// 用户销毁 LP 代币，取回对应的 token_0 和 token_1
pub fn remove_liquidity(
    ctx: Context<RemoveLiquidity>,
    lp_amount: u64,
) -> Result<()> {
    let pair = &mut ctx.accounts.pair;
    let lp_supply = ctx.accounts.lp_mint.supply;
    
    require!(
        lp_amount > 0 && lp_amount <= lp_supply,
        ErrorCode::InsufficientLiquidity
    );
    
    // 先保存 bump，避免借用冲突
    let bump = pair.bump;
    let token_0 = pair.token_0;
    let token_1 = pair.token_1;
    
    // 计算应该返还的代币数量
    // amount_0 = lp_amount * reserve_0 / total_supply
    // amount_1 = lp_amount * reserve_1 / total_supply
    let amount_0 = (lp_amount as u128)
        .checked_mul(pair.reserve_0 as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(lp_supply as u128)
        .ok_or(ErrorCode::MathOverflow)? as u64;
    
    let amount_1 = (lp_amount as u128)
        .checked_mul(pair.reserve_1 as u128)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(lp_supply as u128)
        .ok_or(ErrorCode::MathOverflow)? as u64;
    
    require!(
        amount_0 > 0 && amount_1 > 0,
        ErrorCode::InsufficientLiquidity
    );
    
    // 更新储备量
    pair.reserve_0 = pair.reserve_0
        .checked_sub(amount_0)
        .ok_or(ErrorCode::InsufficientReserves)?;
    pair.reserve_1 = pair.reserve_1
        .checked_sub(amount_1)
        .ok_or(ErrorCode::InsufficientReserves)?;
    
    // 准备 seeds
    let seeds = &[
        b"pair",
        token_0.as_ref(),
        token_1.as_ref(),
        &[bump],
    ];
    let signer = &[&seeds[..]];
    
    // 销毁 LP 代币
    let burn_cpi_accounts = Burn {
        mint: ctx.accounts.lp_mint.to_account_info(),
        from: ctx.accounts.lp_token_account.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let burn_cpi_program = ctx.accounts.token_program.to_account_info();
    let burn_cpi_ctx = CpiContext::new(burn_cpi_program, burn_cpi_accounts);
    token::burn(burn_cpi_ctx, lp_amount)?;
    
    // 返还 token_0
    let transfer_0_cpi_accounts = Transfer {
        from: ctx.accounts.pair_token_0_account.to_account_info(),
        to: ctx.accounts.user_token_0_account.to_account_info(),
        authority: ctx.accounts.pair.to_account_info(),
    };
    let transfer_0_cpi_program = ctx.accounts.token_program.to_account_info();
    let transfer_0_cpi_ctx = CpiContext::new_with_signer(transfer_0_cpi_program, transfer_0_cpi_accounts, signer);
    token::transfer(transfer_0_cpi_ctx, amount_0)?;
    
    // 返还 token_1
    let transfer_1_cpi_accounts = Transfer {
        from: ctx.accounts.pair_token_1_account.to_account_info(),
        to: ctx.accounts.user_token_1_account.to_account_info(),
        authority: ctx.accounts.pair.to_account_info(),
    };
    let transfer_1_cpi_program = ctx.accounts.token_program.to_account_info();
    let transfer_1_cpi_ctx = CpiContext::new_with_signer(transfer_1_cpi_program, transfer_1_cpi_accounts, signer);
    token::transfer(transfer_1_cpi_ctx, amount_1)?;
    
    msg!("移除流动性: 销毁 {} LP, 返还 {} token_0, {} token_1", lp_amount, amount_0, amount_1);
    
    Ok(())
}

