use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::Pair;
use crate::error::ErrorCode;

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    
    /// 交易对的 token_0 账户
    #[account(mut)]
    pub pair_token_0_account: Box<Account<'info, TokenAccount>>,
    
    /// 交易对的 token_1 账户
    #[account(mut)]
    pub pair_token_1_account: Box<Account<'info, TokenAccount>>,
    
    /// 用户的输入代币账户（转出代币）
    #[account(mut)]
    pub user_token_in_account: Box<Account<'info, TokenAccount>>,
    
    /// 用户的输出代币账户（接收代币）
    #[account(mut)]
    pub user_token_out_account: Box<Account<'info, TokenAccount>>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

/// 交换代币（通用函数，支持两个方向）
/// 使用恒定乘积公式：x * y = k
/// 手续费：0.3% (997/1000)
/// 
/// 方向由账户决定：
/// - 如果 user_token_in 是 token_0，则用 token_0 换 token_1
/// - 如果 user_token_in 是 token_1，则用 token_1 换 token_0
pub fn swap(
    ctx: Context<Swap>,
    amount_in: u64,
    min_amount_out: u64,  // 滑点保护：最少能得到的输出代币数量
) -> Result<()> {
    let pair = &mut ctx.accounts.pair;
    
    require!(
        pair.reserve_0 > 0 && pair.reserve_1 > 0,
        ErrorCode::InsufficientLiquidity
    );
    
    // 保存需要的值
    let reserve_0 = pair.reserve_0;
    let reserve_1 = pair.reserve_1;
    let bump = pair.bump;
    let token_0 = pair.token_0;
    let token_1 = pair.token_1;
    
    // 判断交换方向：检查用户的输入代币是 token_0 还是 token_1
    let is_token_0_in = ctx.accounts.user_token_in_account.mint == token_0;
    let is_token_1_in = ctx.accounts.user_token_in_account.mint == token_1;
    
    require!(
        is_token_0_in || is_token_1_in,
        ErrorCode::InvalidTokenMint
    );
    
    // 根据方向确定源和目标储备量
    let (src_reserve, dst_reserve, src_vault, dst_vault) = if is_token_0_in {
        // 用 token_0 换 token_1
        (
            reserve_0,
            reserve_1,
            ctx.accounts.pair_token_0_account.to_account_info(),
            ctx.accounts.pair_token_1_account.to_account_info(),
        )
    } else {
        // 用 token_1 换 token_0
        (
            reserve_1,
            reserve_0,
            ctx.accounts.pair_token_1_account.to_account_info(),
            ctx.accounts.pair_token_0_account.to_account_info(),
        )
    };
    
    // 计算输出数量（恒定乘积公式 + 0.3% 手续费）
    // amount_out = (amount_in * 997 * dst_reserve) / (src_reserve * 1000 + amount_in * 997)
    const FEE_NUMERATOR: u128 = 997;
    const FEE_DENOMINATOR: u128 = 1000;
    
    let amount_in_with_fee = (amount_in as u128)
        .checked_mul(FEE_NUMERATOR)
        .ok_or(ErrorCode::MathOverflow)?;
    
    let numerator = amount_in_with_fee
        .checked_mul(dst_reserve as u128)
        .ok_or(ErrorCode::MathOverflow)?;
    
    let denominator = (src_reserve as u128)
        .checked_mul(FEE_DENOMINATOR)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_add(amount_in_with_fee)
        .ok_or(ErrorCode::MathOverflow)?;
    
    let amount_out = numerator
        .checked_div(denominator)
        .ok_or(ErrorCode::MathOverflow)? as u64;
    
    require!(
        amount_out >= min_amount_out,
        ErrorCode::InsufficientOutputAmount
    );
    
    require!(
        amount_out < dst_reserve,
        ErrorCode::InsufficientLiquidity
    );
    
    // 更新储备量
    if is_token_0_in {
        pair.reserve_0 = reserve_0
            .checked_add(amount_in)
            .ok_or(ErrorCode::MathOverflow)?;
        pair.reserve_1 = reserve_1
            .checked_sub(amount_out)
            .ok_or(ErrorCode::InsufficientReserves)?;
    } else {
        pair.reserve_1 = reserve_1
            .checked_add(amount_in)
            .ok_or(ErrorCode::MathOverflow)?;
        pair.reserve_0 = reserve_0
            .checked_sub(amount_out)
            .ok_or(ErrorCode::InsufficientReserves)?;
    }
    
    // 准备 seeds
    let seeds = &[
        b"pair",
        token_0.as_ref(),
        token_1.as_ref(),
        &[bump],
    ];
    let signer = &[&seeds[..]];
    
    // 接收用户的输入代币
    let transfer_in_cpi_accounts = Transfer {
        from: ctx.accounts.user_token_in_account.to_account_info(),
        to: src_vault,
        authority: ctx.accounts.user.to_account_info(),
    };
    let transfer_in_cpi_program = ctx.accounts.token_program.to_account_info();
    let transfer_in_cpi_ctx = CpiContext::new(transfer_in_cpi_program, transfer_in_cpi_accounts);
    token::transfer(transfer_in_cpi_ctx, amount_in)?;
    
    // 转出输出代币给用户
    let transfer_out_cpi_accounts = Transfer {
        from: dst_vault,
        to: ctx.accounts.user_token_out_account.to_account_info(),
        authority: ctx.accounts.pair.to_account_info(),
    };
    let transfer_out_cpi_program = ctx.accounts.token_program.to_account_info();
    let transfer_out_cpi_ctx = CpiContext::new_with_signer(transfer_out_cpi_program, transfer_out_cpi_accounts, signer);
    token::transfer(transfer_out_cpi_ctx, amount_out)?;
    
    if is_token_0_in {
        msg!("Swap: {} token_0 -> {} token_1", amount_in, amount_out);
    } else {
        msg!("Swap: {} token_1 -> {} token_0", amount_in, amount_out);
    }
    
    Ok(())
}

