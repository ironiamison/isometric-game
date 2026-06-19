use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::{
    load_instruction_at_checked,
    ID as INSTRUCTIONS_SYSVAR_ID,
};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("FQumtv5g3m625Ue3TSsEaeP2igFQMPDxiaek4xunHEd3");

pub const WITHDRAW_PREFIX: &[u8] = b"solstead:withdraw:";

#[program]
pub mod solstead_escrow {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.authority = ctx.accounts.authority.key();
        vault.mint = ctx.accounts.mint.key();
        vault.bump = ctx.bumps.vault;
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, EscrowError::InvalidAmount);

        let cpi_accounts = Transfer {
            from: ctx.accounts.depositor_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.depositor.to_account_info(),
        };
        token::transfer(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts),
            amount,
        )?;

        emit!(DepositEvent {
            depositor: ctx.accounts.depositor.key(),
            amount,
            mint: ctx.accounts.mint.key(),
        });

        Ok(())
    }

    /// Withdraw SPL from the vault to the recipient.
    /// Requires an Ed25519 verify instruction at index 0 that proves the vault
    /// authority signed `solstead:withdraw:{recipient}:{amount}:{expires_at}`.
    pub fn withdraw(
        ctx: Context<Withdraw>,
        amount: u64,
        expires_at: i64,
        ed25519_ix_index: u16,
    ) -> Result<()> {
        require!(amount > 0, EscrowError::InvalidAmount);
        let now = Clock::get()?.unix_timestamp;
        require!(now <= expires_at, EscrowError::WithdrawExpired);

        let vault = &ctx.accounts.vault;
        let recipient = ctx.accounts.recipient.key();
        verify_withdraw_authorization(
            &vault.authority,
            &recipient,
            amount,
            expires_at,
            ed25519_ix_index,
            &ctx.accounts.instructions.to_account_info(),
        )?;

        let seeds = &[b"vault", vault.mint.as_ref(), &[vault.bump]];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_token_account.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                signer,
            ),
            amount,
        )?;

        emit!(WithdrawEvent {
            recipient,
            amount,
            mint: vault.mint,
        });

        Ok(())
    }
}

fn verify_withdraw_authorization(
    authority: &Pubkey,
    recipient: &Pubkey,
    amount: u64,
    expires_at: i64,
    ed25519_ix_index: u16,
    instructions_sysvar: &AccountInfo,
) -> Result<()> {
    let message = format!("solstead:withdraw:{recipient}:{amount}:{expires_at}");

    let ed25519_ix = load_instruction_at_checked(
        ed25519_ix_index as usize,
        instructions_sysvar,
    )
    .map_err(|_| error!(EscrowError::MissingEd25519Instruction))?;

    require!(
        ed25519_ix.program_id == anchor_lang::solana_program::ed25519_program::ID,
        EscrowError::MissingEd25519Instruction
    );
    require!(ed25519_ix.data.len() >= 112, EscrowError::InvalidEd25519Instruction);

    let sig_offset = u16::from_le_bytes(ed25519_ix.data[0..2].try_into().unwrap()) as usize;
    let sig_ix_index = u16::from_le_bytes(ed25519_ix.data[2..4].try_into().unwrap());
    let pubkey_offset = u16::from_le_bytes(ed25519_ix.data[4..6].try_into().unwrap()) as usize;
    let pubkey_ix_index = u16::from_le_bytes(ed25519_ix.data[6..8].try_into().unwrap());
    let message_offset = u16::from_le_bytes(ed25519_ix.data[8..10].try_into().unwrap()) as usize;
    let message_size = u16::from_le_bytes(ed25519_ix.data[10..12].try_into().unwrap()) as usize;
    let message_ix_index = u16::from_le_bytes(ed25519_ix.data[12..14].try_into().unwrap());

    require!(sig_ix_index == u16::MAX, EscrowError::InvalidEd25519Instruction);
    require!(pubkey_ix_index == u16::MAX, EscrowError::InvalidEd25519Instruction);
    require!(message_ix_index == u16::MAX, EscrowError::InvalidEd25519Instruction);

    let expected_pubkey = authority.to_bytes();
    let actual_pubkey = &ed25519_ix.data[pubkey_offset..pubkey_offset + 32];
    require!(
        actual_pubkey == expected_pubkey,
        EscrowError::InvalidWithdrawAuthority
    );

    let actual_message = &ed25519_ix.data[message_offset..message_offset + message_size];
    require!(
        actual_message == message.as_bytes(),
        EscrowError::InvalidWithdrawMessage
    );

    let _signature = &ed25519_ix.data[sig_offset..sig_offset + 64];
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        space = 8 + VaultState::INIT_SPACE,
        seeds = [b"vault", mint.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, VaultState>,

    #[account(
        init,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    pub depositor: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        seeds = [b"vault", mint.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, VaultState>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = depositor,
    )]
    pub depositor_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    /// CHECK: Recipient wallet; validated against signed withdraw message.
    pub recipient: UncheckedAccount<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        seeds = [b"vault", mint.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, VaultState>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = recipient,
    )]
    pub recipient_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Instructions sysvar for Ed25519 verify instruction.
    #[account(address = INSTRUCTIONS_SYSVAR_ID)]
    pub instructions: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct VaultState {
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub bump: u8,
}

#[event]
pub struct DepositEvent {
    pub depositor: Pubkey,
    pub amount: u64,
    pub mint: Pubkey,
}

#[event]
pub struct WithdrawEvent {
    pub recipient: Pubkey,
    pub amount: u64,
    pub mint: Pubkey,
}

#[error_code]
pub enum EscrowError {
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Withdraw authorization expired")]
    WithdrawExpired,
    #[msg("Missing Ed25519 verify instruction")]
    MissingEd25519Instruction,
    #[msg("Invalid Ed25519 verify instruction")]
    InvalidEd25519Instruction,
    #[msg("Withdraw authority mismatch")]
    InvalidWithdrawAuthority,
    #[msg("Withdraw message mismatch")]
    InvalidWithdrawMessage,
}
