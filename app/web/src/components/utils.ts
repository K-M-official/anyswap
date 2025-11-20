import { Connection, PublicKey, Transaction } from '@solana/web3.js';
import { 
  createMint, 
  mintTo, 
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID
} from '@solana/spl-token';

/**
 * @description 创建 Token, 使用 SPL Token 标准库创建
 * 支持钱包适配器（使用 signTransaction）
 */
export async function handleCreateToken(
  connection: Connection,
  publicKey: PublicKey,
  signTransaction: (tx: Transaction) => Promise<Transaction>,
  tokenName: string,
  tokenSymbol: string,
  tokenDecimals: number
): Promise<{ mint: PublicKey; signature: string; name: string; symbol: string; decimals: number }> {
  if (!tokenName || !tokenSymbol) {
    throw new Error('请填写完整的 Token 信息（名称、符号、小数位数）');
  }

  if (tokenDecimals < 0 || tokenDecimals > 9) {
    throw new Error('小数位数必须是 0-9 之间的数字');
  }

  // 创建钱包适配器包装器
  const walletAdapter = {
    publicKey: publicKey,
    signTransaction: signTransaction,
  };

  // 使用 SPL Token 标准库创建 mint
  // createMint 内部会创建账户、发送交易并确认
  const mint = await createMint(
    connection,
    walletAdapter as any, // payer (钱包适配器)
    publicKey, // mint authority
    null, // freeze authority (null = no freeze authority)
    tokenDecimals // decimals
  );

  // 注意：createMint 已经发送并确认了交易
  // 我们可以通过查询最近的交易来获取签名，但这里简化处理
  // 实际使用中，如果需要签名，可以在调用前手动构建交易
  
  return {
    mint: mint,
    signature: '', // createMint 内部已经发送交易，这里返回空字符串
    name: tokenName,
    symbol: tokenSymbol,
    decimals: tokenDecimals,
  };
}

/**
 * Mint Token 给自己的封装函数（组件级别）
 * 使用 SPL Token 标准库实现
 */
export async function handleMintToken(
  connection: Connection,
  publicKey: PublicKey,
  signTransaction: (tx: Transaction) => Promise<Transaction>,
  mintAddress: string,
  amount: number,
  decimals: number
): Promise<{ signature: string; userTokenAccount: string; amount: string }> {
  if (!mintAddress || !amount || amount <= 0) {
    throw new Error('请选择 Token 并输入有效的数量（大于 0）');
  }

  const mint = new PublicKey(mintAddress);
  
  // 获取或创建用户的关联代币账户
  const userTokenAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    { publicKey, signTransaction } as any, // payer
    mint,
    publicKey, // owner
    false, // allowOwnerOffCurve
    undefined, // commitment
    undefined, // confirmOptions
    TOKEN_PROGRAM_ID
  );

  // 计算要铸造的数量（考虑小数位）
  const amountInSmallestUnit = BigInt(Math.floor(amount * Math.pow(10, decimals)));

  // 使用 SPL Token 标准库铸造代币
  const signature = await mintTo(
    connection,
    { publicKey, signTransaction } as any, // payer
    mint,
    userTokenAccount.address,
    publicKey, // mint authority (用户自己)
    amountInSmallestUnit, // amount
    [], // multiSigners
    undefined, // confirmOptions
    TOKEN_PROGRAM_ID
  );

  return {
    signature: signature,
    userTokenAccount: userTokenAccount.address.toString(),
    amount: amountInSmallestUnit.toString(),
  };
}

