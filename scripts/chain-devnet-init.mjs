#!/usr/bin/env node
/**
 * Initialize the Solstead escrow vault on devnet after anchor deploy.
 * Usage: node scripts/chain-devnet-init.mjs <programId> <mint> <authorityKeypairPath>
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import anchor from "@coral-xyz/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");

const [programIdStr, mintStr, authorityPath] = process.argv.slice(2);
if (!programIdStr || !mintStr || !authorityPath) {
  console.error(
    "Usage: node scripts/chain-devnet-init.mjs <programId> <mint> <authorityKeypairPath>"
  );
  process.exit(1);
}

const programId = new PublicKey(programIdStr);
const mint = new PublicKey(mintStr);
const authority = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync(authorityPath, "utf8")))
);

const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const wallet = new anchor.Wallet(authority);
const provider = new anchor.AnchorProvider(connection, wallet, {
  commitment: "confirmed",
});
anchor.setProvider(provider);

const idl = JSON.parse(
  fs.readFileSync(path.join(root, "target/idl/solstead_escrow.json"), "utf8")
);
const program = new anchor.Program(idl, provider);

const [vault] = PublicKey.findProgramAddressSync(
  [Buffer.from("vault"), mint.toBuffer()],
  programId
);
const vaultTokenAccount = getAssociatedTokenAddressSync(mint, vault, true);

const sig = await program.methods
  .initialize()
  .accounts({
    authority: authority.publicKey,
    mint,
    vault,
    vaultTokenAccount,
    tokenProgram: TOKEN_PROGRAM_ID,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  })
  .rpc();

console.log("Vault initialized:", sig);
console.log("Vault PDA:", vault.toBase58());
console.log("Vault token account:", vaultTokenAccount.toBase58());
