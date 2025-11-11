import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Fucksol2 } from "../target/types/fucksol2";
import * as token from "@solana/spl-token";
import { PublicKey, Keypair } from "@solana/web3.js";
import { expect } from "chai";

interface Pair {
  token0: PublicKey;
  token1: PublicKey;
  pair: PublicKey;
  lpMint: PublicKey;
  pairToken0Account: PublicKey;
  pairToken1Account: PublicKey;
}

interface LPProvider {
  signer: Keypair;
  user0: PublicKey;
  user1: PublicKey;
  lpAccount: PublicKey;
}

describe("fucksol2", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.fucksol2 as Program<Fucksol2>;
  const connection = provider.connection;
  const payer = provider.wallet as anchor.Wallet;

  let pair: Pair;
  let mintAuth: Keypair;
  let n_decimals = 9;

  async function get_token_balance(pk: PublicKey) {
    return (await connection.getTokenAccountBalance(pk)).value.uiAmount;
  }

  function lp_amount(n: number) {
    return new anchor.BN(n * 10 ** n_decimals);
  }

  it("创建交易对", async () => {
    const auth = Keypair.generate();
    const sig = await connection.requestAirdrop(auth.publicKey, 100 * anchor.web3.LAMPORTS_PER_SOL);
    await connection.confirmTransaction(sig);

    const mint0 = await token.createMint(
      connection,
      auth,
      auth.publicKey,
      auth.publicKey,
      n_decimals
    );
    const mint1 = await token.createMint(
      connection,
      auth,
      auth.publicKey,
      auth.publicKey,
      n_decimals
    );

    const token0 = mint0;
    const token1 = mint1;

    const [pairPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("pair"), token0.toBuffer(), token1.toBuffer()],
      program.programId
    );
    const [lpMintPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("lp_mint"), token0.toBuffer(), token1.toBuffer()],
      program.programId
    );

    const pairToken0Keypair = Keypair.generate();
    const pairToken1Keypair = Keypair.generate();
    const lamports = await connection.getMinimumBalanceForRentExemption(165);

    const createPairToken0Tx = new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: pairToken0Keypair.publicKey,
        space: 165,
        lamports,
        programId: token.TOKEN_PROGRAM_ID,
      }),
      token.createInitializeAccountInstruction(
        pairToken0Keypair.publicKey,
        token0,
        pairPda,
        token.TOKEN_PROGRAM_ID
      )
    );

    const createPairToken1Tx = new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: pairToken1Keypair.publicKey,
        space: 165,
        lamports,
        programId: token.TOKEN_PROGRAM_ID,
      }),
      token.createInitializeAccountInstruction(
        pairToken1Keypair.publicKey,
        token1,
        pairPda,
        token.TOKEN_PROGRAM_ID
      )
    );

    await provider.sendAndConfirm(createPairToken0Tx, [pairToken0Keypair]);
    await provider.sendAndConfirm(createPairToken1Tx, [pairToken1Keypair]);

    await program.methods
      .createPair(token0, token1)
      .accounts({
        authority: payer.publicKey,
      })
      .rpc();

    pair = {
      token0: token0,
      token1: token1,
      pair: pairPda,
      lpMint: lpMintPda,
      pairToken0Account: pairToken0Keypair.publicKey,
      pairToken1Account: pairToken1Keypair.publicKey,
    };
    mintAuth = auth;

    const pairAccount = await program.account.pair.fetch(pairPda);
    expect(pairAccount.token0.toString()).to.equal(token0.toString());
    expect(pairAccount.token1.toString()).to.equal(token1.toString());
    expect(pairAccount.lpMint.toString()).to.equal(lpMintPda.toString());
    expect(pairAccount.reserve0.toNumber()).to.equal(0);
    expect(pairAccount.reserve1.toNumber()).to.equal(0);
  });

  let lp_user0: LPProvider;

  it("添加流动性（首次）", async () => {
    const lp_user_signer = Keypair.generate();
    const lp_user = lp_user_signer.publicKey;
    const sig = await connection.requestAirdrop(lp_user, 100 * anchor.web3.LAMPORTS_PER_SOL);
    await connection.confirmTransaction(sig);

    const user0 = await token.createAssociatedTokenAccount(
      connection,
      (payer as any).payer as anchor.web3.Signer,
      pair.token0,
      lp_user
    );
    const user1 = await token.createAssociatedTokenAccount(
      connection,
      (payer as any).payer as anchor.web3.Signer,
      pair.token1,
      lp_user
    );
    const lpAccount = await token.createAssociatedTokenAccount(
      connection,
      (payer as any).payer as anchor.web3.Signer,
      pair.lpMint,
      lp_user
    );


    await token.mintTo(
      connection,
      mintAuth,
      pair.token0,
      user0,
      mintAuth.publicKey,
      100 * 10 ** n_decimals
    );
    await token.mintTo(
      connection,
      mintAuth,
      pair.token1,
      user1,
      mintAuth.publicKey,
      100 * 10 ** n_decimals
    );

    const amount0 = lp_amount(50);
    const amount1 = lp_amount(50);

    await program.methods
      .addLiquidity(amount0, amount1)
      .accounts({
        pair: pair.pair,
        lpMint: pair.lpMint,
        lpTokenAccount: lpAccount,
        pairToken0Account: pair.pairToken0Account,
        pairToken1Account: pair.pairToken1Account,
        userToken0Account: user0,
        userToken1Account: user1,
        user: lp_user,
      })
      .signers([lp_user_signer])
      .rpc();

    lp_user0 = {
      signer: lp_user_signer,
      user0: user0,
      user1: user1,
      lpAccount: lpAccount,
    };

    const balance_lp = await get_token_balance(lpAccount);
    const pairAccount = await program.account.pair.fetch(pair.pair);
    const reserve0 = pairAccount.reserve0.toNumber();
    const reserve1 = pairAccount.reserve1.toNumber();

    console.log("首次添加流动性 LP 数量:", balance_lp);
    console.log("储备量 - token0:", reserve0);
    console.log("储备量 - token1:", reserve1);

    expect(balance_lp).to.be.greaterThan(0);
    expect(reserve0).to.equal(amount0.toNumber());
    expect(reserve1).to.equal(amount1.toNumber());
  });

  it("执行 Swap (token0 -> token1)", async () => {
    const swapper_signer = Keypair.generate();
    const swapper = swapper_signer.publicKey;
    const sig = await connection.requestAirdrop(swapper, 100 * anchor.web3.LAMPORTS_PER_SOL);
    await connection.confirmTransaction(sig);

    const mint0_ata = await token.createAssociatedTokenAccount(
      connection,
      (payer as any).payer as anchor.web3.Signer,
      pair.token0,
      swapper
    );
    const mint1_ata = await token.createAssociatedTokenAccount(
      connection,
      (payer as any).payer as anchor.web3.Signer,
      pair.token1,
      swapper
    );


    await token.mintTo(
      connection,
      mintAuth,
      pair.token0,
      mint0_ata,
      mintAuth.publicKey,
      100 * 10 ** n_decimals
    );

    const b0 = await get_token_balance(mint0_ata);
    const b1 = await get_token_balance(mint1_ata);

    await program.methods
      .swap(new anchor.BN(10 * 10 ** n_decimals), new anchor.BN(0))
      .accounts({
        pair: pair.pair,
        pairToken0Account: pair.pairToken0Account,
        pairToken1Account: pair.pairToken1Account,
        userTokenInAccount: mint0_ata,
        userTokenOutAccount: mint1_ata,
        user: swapper,
      })
      .signers([swapper_signer])
      .rpc();

    const new_b0 = await get_token_balance(mint0_ata);
    const new_b1 = await get_token_balance(mint1_ata);

    expect(new_b0).to.be.lessThan(b0);
    expect(new_b1).to.be.greaterThan(b1);
  });

  it("移除流动性", async () => {
    const b_user0 = await get_token_balance(lp_user0.user0);
    const b_user1 = await get_token_balance(lp_user0.user1);
    const balance_lp = await get_token_balance(lp_user0.lpAccount);

    const lpBalanceInfo = await connection.getTokenAccountBalance(lp_user0.lpAccount);
    const burnAmount = new anchor.BN(lpBalanceInfo.value.amount).div(new anchor.BN(2));

    await program.methods
      .removeLiquidity(burnAmount)
      .accounts({
        pair: pair.pair,
        lpMint: pair.lpMint,
        lpTokenAccount: lp_user0.lpAccount,
        pairToken0Account: pair.pairToken0Account,
        pairToken1Account: pair.pairToken1Account,
        userToken0Account: lp_user0.user0,
        userToken1Account: lp_user0.user1,
        user: lp_user0.signer.publicKey,
      })
      .signers([lp_user0.signer])
      .rpc();

    const b_user0_2 = await get_token_balance(lp_user0.user0);
    const b_user1_2 = await get_token_balance(lp_user0.user1);
    const balance_lp_2 = await get_token_balance(lp_user0.lpAccount);

    expect(balance_lp).to.be.greaterThan(balance_lp_2);
    expect(b_user0).to.be.lessThan(b_user0_2);
    expect(b_user1).to.be.lessThan(b_user1_2);
  });

  let lp_user1: LPProvider;

  it("添加第二次流动性", async () => {
    const lp_user_signer = Keypair.generate();
    const lp_user = lp_user_signer.publicKey;
    const sig = await connection.requestAirdrop(lp_user, 100 * anchor.web3.LAMPORTS_PER_SOL);
    await connection.confirmTransaction(sig);

    const user0 = await token.createAssociatedTokenAccount(
      connection,
      (payer as any).payer as anchor.web3.Signer,
      pair.token0,
      lp_user
    );
    const user1 = await token.createAssociatedTokenAccount(
      connection,
      (payer as any).payer as anchor.web3.Signer,
      pair.token1,
      lp_user
    );
    const lpAccount = await token.createAssociatedTokenAccount(
      connection,
      (payer as any).payer as anchor.web3.Signer,
      pair.lpMint,
      lp_user
    );


    await token.mintTo(
      connection,
      mintAuth,
      pair.token0,
      user0,
      mintAuth.publicKey,
      100 * 10 ** n_decimals
    );
    await token.mintTo(
      connection,
      mintAuth,
      pair.token1,
      user1,
      mintAuth.publicKey,
      100 * 10 ** n_decimals
    );

    const pairAccount = await program.account.pair.fetch(pair.pair);
    const reserve0 = pairAccount.reserve0.toNumber();
    const reserve1 = pairAccount.reserve1.toNumber();

    const amount0 = lp_amount(25);
    const requiredAmount1 = new anchor.BN((amount0.toNumber() * reserve1) / reserve0);
    const amount1 = requiredAmount1;

    await program.methods
      .addLiquidity(amount0, amount1)
      .accounts({
        pair: pair.pair,
        lpMint: pair.lpMint,
        lpTokenAccount: lpAccount,
        pairToken0Account: pair.pairToken0Account,
        pairToken1Account: pair.pairToken1Account,
        userToken0Account: user0,
        userToken1Account: user1,
        user: lp_user,
      })
      .signers([lp_user_signer])
      .rpc();

    lp_user1 = {
      signer: lp_user_signer,
      user0: user0,
      user1: user1,
      lpAccount: lpAccount,
    };

    const balance_lp = await get_token_balance(lpAccount);
    const pairAccount2 = await program.account.pair.fetch(pair.pair);
    const reserve0_2 = pairAccount2.reserve0.toNumber();
    const reserve1_2 = pairAccount2.reserve1.toNumber();

    console.log("第二次添加流动性 LP 数量:", balance_lp);
    console.log("储备量 - token0:", reserve0_2);
    console.log("储备量 - token1:", reserve1_2);

    expect(balance_lp).to.be.greaterThan(0);
  });

  it("Swap 后移除流动性", async () => {
    const b_user0 = await get_token_balance(lp_user1.user0);
    const b_user1 = await get_token_balance(lp_user1.user1);
    const balance_lp = await get_token_balance(lp_user1.lpAccount);

    const lpBalanceInfo = await connection.getTokenAccountBalance(lp_user1.lpAccount);
    const burnAmount = new anchor.BN(lpBalanceInfo.value.amount);

    await program.methods
      .removeLiquidity(burnAmount)
      .accounts({
        pair: pair.pair,
        lpMint: pair.lpMint,
        lpTokenAccount: lp_user1.lpAccount,
        pairToken0Account: pair.pairToken0Account,
        pairToken1Account: pair.pairToken1Account,
        userToken0Account: lp_user1.user0,
        userToken1Account: lp_user1.user1,
        user: lp_user1.signer.publicKey,
      })
      .signers([lp_user1.signer])
      .rpc();

    const b_user0_2 = await get_token_balance(lp_user1.user0);
    const b_user1_2 = await get_token_balance(lp_user1.user1);
    const balance_lp_2 = await get_token_balance(lp_user1.lpAccount);

    expect(balance_lp).to.be.greaterThan(balance_lp_2);
    expect(b_user0).to.be.lessThan(b_user0_2);
    expect(b_user1).to.be.lessThan(b_user1_2);
  });
});
