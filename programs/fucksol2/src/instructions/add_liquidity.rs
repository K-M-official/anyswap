use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};
use crate::state::Pair;
use crate::error::ErrorCode;

/// 计算整数平方根（Babylonian method）
fn integer_sqrt(n: u128) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x as u64
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub pair: Account<'info, Pair>,
    
    #[account(
        mut,
        address = pair.lp_mint @ ErrorCode::InvalidLpMint
    )]
    pub lp_mint: Account<'info, Mint>,
    
    /// 用户的 LP 代币账户（接收新铸造的 LP）
    #[account(mut)]
    pub lp_token_account: Box<Account<'info, TokenAccount>>,
    
    /// 交易对的 token_0 账户（接收用户转入的代币）
    #[account(mut)]
    pub pair_token_0_account: Box<Account<'info, TokenAccount>>,
    
    /// 交易对的 token_1 账户（接收用户转入的代币）
    #[account(mut)]
    pub pair_token_1_account: Box<Account<'info, TokenAccount>>,
    
    /// 用户的 token_0 账户（转出代币）
    #[account(mut)]
    pub user_token_0_account: Box<Account<'info, TokenAccount>>,
    
    /// 用户的 token_1 账户（转出代币）
    #[account(mut)]
    pub user_token_1_account: Box<Account<'info, TokenAccount>>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

/// 添加流动性
/// 用户提供 token_0 和 token_1，获得 LP 代币作为凭证
pub fn add_liquidity(
    ctx: Context<AddLiquidity>,
    amount_0: u64,
    amount_1: u64,
) -> Result<()> {
    let pair = &mut ctx.accounts.pair;
    
    // 先保存需要的值，避免借用冲突
    let token_0 = pair.token_0;
    let token_1 = pair.token_1;
    let bump = pair.bump;
    let is_first_liquidity = pair.reserve_0 == 0 && pair.reserve_1 == 0;
    
    // 先接收用户的代币（从用户账户转到 pair 账户）
    // 转账 token_0
    let transfer_0_cpi_accounts = token::Transfer {
        from: ctx.accounts.user_token_0_account.to_account_info(),
        to: ctx.accounts.pair_token_0_account.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let transfer_0_cpi_program = ctx.accounts.token_program.to_account_info();
    let transfer_0_cpi_ctx = CpiContext::new(transfer_0_cpi_program, transfer_0_cpi_accounts);
    token::transfer(transfer_0_cpi_ctx, amount_0)?;
    
    // 转账 token_1
    let transfer_1_cpi_accounts = token::Transfer {
        from: ctx.accounts.user_token_1_account.to_account_info(),
        to: ctx.accounts.pair_token_1_account.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let transfer_1_cpi_program = ctx.accounts.token_program.to_account_info();
    let transfer_1_cpi_ctx = CpiContext::new(transfer_1_cpi_program, transfer_1_cpi_accounts);
    token::transfer(transfer_1_cpi_ctx, amount_1)?;
    
    // 准备 seeds（用于后续的 CPI 调用）
    let seeds = &[
        b"pair",
        token_0.as_ref(),
        token_1.as_ref(),
        &[bump],
    ];
    let signer = &[&seeds[..]];
    
    // 如果是第一次添加流动性，直接使用提供的数量
    // 否则需要按比例添加
    if is_first_liquidity {
        // 第一次添加流动性
        pair.reserve_0 = amount_0;
        pair.reserve_1 = amount_1;
        
        // 第一次添加时，LP 代币数量 = sqrt(amount_0 * amount_1) - MINIMUM_LIQUIDITY
        // MINIMUM_LIQUIDITY 防止第一次添加后立即移除所有流动性
        const MINIMUM_LIQUIDITY: u64 = 1000; // 最小流动性，防止除零
        let product = (amount_0 as u128)
            .checked_mul(amount_1 as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        let sqrt = integer_sqrt(product);
        let lp_amount = sqrt
            .checked_sub(MINIMUM_LIQUIDITY)
            .ok_or(ErrorCode::InsufficientLiquidity)?;
        
        // 铸造 LP 代币给用户
        let cpi_accounts = MintTo {
            mint: ctx.accounts.lp_mint.to_account_info(),
            to: ctx.accounts.lp_token_account.to_account_info(),
            authority: ctx.accounts.pair.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::mint_to(cpi_ctx, lp_amount)?;
        
        msg!("首次添加流动性: {} token_0, {} token_1, 获得 {} LP", amount_0, amount_1, lp_amount);
    } else {
        // 后续添加流动性，需要按比例
        // 先保存储备量用于计算
        let reserve_0 = pair.reserve_0;
        let reserve_1 = pair.reserve_1;
        
        // 计算应该添加的 token_1 数量：amount_1 = amount_0 * reserve_1 / reserve_0
        let required_amount_1 = (amount_0 as u128)
            .checked_mul(reserve_1 as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(reserve_0 as u128)
            .ok_or(ErrorCode::MathOverflow)? as u64;
        
        require!(
            amount_1 >= required_amount_1,
            ErrorCode::InsufficientTokenAmount
        );
        
        // 计算应该铸造的 LP 代币数量
        // LP = total_supply * min(amount_0/reserve_0, amount_1/reserve_1)
        let lp_supply = ctx.accounts.lp_mint.supply;
        let lp_from_0 = (lp_supply as u128)
            .checked_mul(amount_0 as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(reserve_0 as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        let lp_from_1 = (lp_supply as u128)
            .checked_mul(amount_1 as u128)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(reserve_1 as u128)
            .ok_or(ErrorCode::MathOverflow)?;
        
        let lp_amount = lp_from_0.min(lp_from_1) as u64;
        
        // 更新储备量
        pair.reserve_0 = reserve_0
            .checked_add(amount_0)
            .ok_or(ErrorCode::MathOverflow)?;
        pair.reserve_1 = reserve_1
            .checked_add(amount_1)
            .ok_or(ErrorCode::MathOverflow)?;
        
        // 铸造 LP 代币
        let cpi_accounts = MintTo {
            mint: ctx.accounts.lp_mint.to_account_info(),
            to: ctx.accounts.lp_token_account.to_account_info(),
            authority: ctx.accounts.pair.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::mint_to(cpi_ctx, lp_amount)?;
        
        msg!("添加流动性: {} token_0, {} token_1, 获得 {} LP", amount_0, amount_1, lp_amount);
    }
    
    Ok(())
}

