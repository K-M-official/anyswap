use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("代币顺序无效：token_0 的地址必须小于 token_1")]
    InvalidTokenOrder,
    #[msg("LP Mint 地址不匹配")]
    InvalidLpMint,
    #[msg("数学运算溢出")]
    MathOverflow,
    #[msg("流动性不足")]
    InsufficientLiquidity,
    #[msg("代币数量不足")]
    InsufficientTokenAmount,
    #[msg("储备量不足")]
    InsufficientReserves,
    #[msg("输出数量不足（滑点过大）")]
    InsufficientOutputAmount,
    #[msg("无效的代币 mint 地址")]
    InvalidTokenMint,
}

