use anchor_lang::prelude::*;

/// 交易对账户结构
/// 存储两个代币的地址和储备量
#[account]
pub struct Pair {
    pub token_0: Pubkey,      // 第一个代币的 mint 地址（地址较小的）
    pub token_1: Pubkey,      // 第二个代币的 mint 地址（地址较大的）
    pub lp_mint: Pubkey,      // LP 代币的 mint 地址
    pub reserve_0: u64,       // token_0 的储备量
    pub reserve_1: u64,       // token_1 的储备量
    pub bump: u8,             // PDA bump seed
}

impl Pair {
    pub const LEN: usize = 8 +      // discriminator
                          32 +      // token_0
                          32 +      // token_1
                          32 +      // lp_mint
                          8 +       // reserve_0
                          8 +       // reserve_1
                          1;        // bump
}

