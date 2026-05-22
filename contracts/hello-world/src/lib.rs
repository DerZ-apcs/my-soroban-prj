// lib.rs
//! # Transparent Volunteer Fund
//! A DAO-like escrow smart contract built on Stellar Soroban.
//! Donors contribute tokens, the Admin creates withdrawal proposals,
//! and donors vote to approve fund releases.

#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    token, symbol_short,
    Address, Env, String, Vec,
};
// hello world fix
// ---------------------------------------------------------------------------
// Storage Keys
// ---------------------------------------------------------------------------

/// Top-level keys for instance/persistent storage.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Address of the contract administrator.
    Admin,
    /// Human-readable name of the fundraising campaign.
    CampaignName,
    /// Address of the accepted donation token (e.g. XLM testnet).
    TokenAddress,
    /// Amount donated by a specific address.
    DonorBalance(Address),
    /// List of all donor addresses (for quorum checks).
    DonorList,
    /// A single proposal, keyed by its numeric ID.
    Proposal(u32),
    /// Monotonically-increasing counter for proposal IDs.
    ProposalCount,
}

// ---------------------------------------------------------------------------
// Domain Types
// ---------------------------------------------------------------------------

/// Current lifecycle state of a withdrawal proposal.
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum ProposalStatus {
    /// Accepting votes.
    Pending,
    /// Quorum reached — funds transferred to admin.
    Executed,
    /// Manually cancelled by admin (future extension).
    Cancelled,
}

/// A single withdrawal proposal created by the admin.
#[contracttype]
#[derive(Clone)]
pub struct Proposal {
    /// Auto-assigned sequential identifier.
    pub id: u32,
    /// Short human-readable purpose (e.g. "Buy supplies").
    pub description: String,
    /// Token amount (in stroops / base units) requested.
    pub amount: i128,
    /// Current lifecycle state.
    pub status: ProposalStatus,
    /// Addresses that have voted YES.
    pub yes_votes: Vec<Address>,
}

// ---------------------------------------------------------------------------
// Error Codes
// ---------------------------------------------------------------------------
// fix loi missing wasm
#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum FundError {
    /// Caller is not the designated admin.
    Unauthorized       = 1,
    /// Contract has already been initialized.
    AlreadyInitialized = 2,
    /// Donation amount must be greater than zero.
    ZeroDonation       = 3,
    /// No proposal exists with the given ID.
    ProposalNotFound   = 4,
    /// Proposal is not in Pending state.
    ProposalNotPending = 5,
    /// Caller has already cast a YES vote on this proposal.
    AlreadyVoted       = 6,
    /// Caller has never donated and may not vote.
    NotADonor          = 7,
    /// Vault holds less than the requested withdrawal amount.
    InsufficientFunds  = 8,
    /// Proposal has not yet reached the minimum approval threshold.
    QuorumNotReached   = 9,
}

// ---------------------------------------------------------------------------
// Helper — minimum yes-votes needed to execute (MVP: 1 donor)
// ---------------------------------------------------------------------------
const QUORUM_MIN: u32 = 1;

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct VolunteerFund;

#[contractimpl]
impl VolunteerFund {

    // -----------------------------------------------------------------------
    // 1. INITIALIZE
    // -----------------------------------------------------------------------

    /// Set the admin, campaign name, and accepted token address.
    /// Can only be called once.
    ///
    /// # Arguments
    /// * `admin`         – The address that owns this contract.
    /// * `name` – A display name for the fund.
    /// * `token`         – SAC-compliant token to accept for donations.
    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        token: Address,
    ) -> Result<(), FundError> {
        // Guard against re-initialization.
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(FundError::AlreadyInitialized);
        }

        // The admin must authorise this setup transaction.
        admin.require_auth();

        let storage = env.storage().instance();
        storage.set(&DataKey::Admin,         &admin);
        storage.set(&DataKey::CampaignName,  &name);
        storage.set(&DataKey::TokenAddress,  &token);
        storage.set(&DataKey::ProposalCount, &0_u32);

        // Initialise an empty donor list in persistent storage.
        let empty: Vec<Address> = Vec::new(&env);
        env.storage().persistent().set(&DataKey::DonorList, &empty);

        env.events().publish(
            (symbol_short!("init"), symbol_short!("fund")),
            (admin, name),
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // 2. DONATE
    // -----------------------------------------------------------------------

    /// Transfer `amount` of the accepted token from `donor` into this contract.
    ///
    /// * The donor's running balance is updated.
    /// * New donors are appended to the global donor list (used for quorum).
    ///
    /// # Arguments
    /// * `donor`  – Address funding the campaign.
    /// * `amount` – Token base-units to deposit (must be > 0).
    pub fn donate(
        env: Env,
        donor: Address,
        amount: i128,
    ) -> Result<(), FundError> {
        if amount <= 0 {
            return Err(FundError::ZeroDonation);
        }

        // The donor must sign this transaction.
        donor.require_auth();

        // Pull accepted token address from instance storage.
        let token_addr: Address = env.storage().instance()
            .get(&DataKey::TokenAddress)
            .unwrap();

        // Transfer from donor wallet → this contract.
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(&donor, &env.current_contract_address(), &amount);

        // Update this donor's recorded balance.
        let balance_key = DataKey::DonorBalance(donor.clone());
        let prev_balance: i128 = env.storage().persistent()
            .get(&balance_key)
            .unwrap_or(0);
        let new_balance = prev_balance + amount;
        env.storage().persistent().set(&balance_key, &new_balance);

        // Register first-time donors in the global list.
        if prev_balance == 0 {
            let mut donors: Vec<Address> = env.storage().persistent()
                .get(&DataKey::DonorList)
                .unwrap_or_else(|| Vec::new(&env));
            donors.push_back(donor.clone());
            env.storage().persistent().set(&DataKey::DonorList, &donors);
        }

        env.events().publish(
            (symbol_short!("donate"),),
            (donor, amount),
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // 3. CREATE PROPOSAL
    // -----------------------------------------------------------------------

    /// Admin creates a withdrawal proposal for a specific milestone.
    ///
    /// # Arguments
    /// * `caller`      – Must match the stored admin address.
    /// * `description` – Short human-readable label for the milestone.
    /// * `amount`      – Token base-units requested.
    ///
    /// Returns the newly created proposal ID.
    pub fn create_proposal(
        env: Env,
        caller: Address,
        description: String,
        amount: i128,
    ) -> Result<u32, FundError> {
        // Only the admin may create proposals.
        let admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .unwrap();
        if caller != admin {
            return Err(FundError::Unauthorized);
        }
        caller.require_auth();

        // Assign and bump the proposal counter.
        let mut count: u32 = env.storage().instance()
            .get(&DataKey::ProposalCount)
            .unwrap_or(0);
        let proposal_id = count;
        count += 1;
        env.storage().instance().set(&DataKey::ProposalCount, &count);

        // Persist the new proposal.
        let proposal = Proposal {
            id:          proposal_id,
            description: description.clone(),
            amount,
            status:      ProposalStatus::Pending,
            yes_votes:   Vec::new(&env),
        };
        env.storage().persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish(
            (symbol_short!("proposal"), symbol_short!("create")),
            (proposal_id, description, amount),
        );

        Ok(proposal_id)
    }

    // -----------------------------------------------------------------------
    // 4. VOTE
    // -----------------------------------------------------------------------

    /// A donor casts a YES vote on a pending proposal.
    ///
    /// Rules:
    /// * Caller must have a recorded donation balance > 0.
    /// * Proposal must be in `Pending` state.
    /// * Each address may vote at most once per proposal.
    ///
    /// # Arguments
    /// * `voter`       – Donor casting the vote.
    /// * `proposal_id` – Target proposal.
    pub fn vote(
        env: Env,
        voter: Address,
        proposal_id: u32,
    ) -> Result<(), FundError> {
        voter.require_auth();

        // Confirm the voter has actually donated.
        let balance: i128 = env.storage().persistent()
            .get(&DataKey::DonorBalance(voter.clone()))
            .unwrap_or(0);
        if balance == 0 {
            return Err(FundError::NotADonor);
        }

        // Load the proposal.
        let mut proposal: Proposal = env.storage().persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(FundError::ProposalNotFound)?;

        // Only Pending proposals accept new votes.
        if proposal.status != ProposalStatus::Pending {
            return Err(FundError::ProposalNotPending);
        }

        // Prevent double-voting.
        for existing in proposal.yes_votes.iter() {
            if existing == voter {
                return Err(FundError::AlreadyVoted);
            }
        }

        // Record the vote.
        proposal.yes_votes.push_back(voter.clone());
        env.storage().persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish(
            (symbol_short!("vote"),),
            (voter, proposal_id, proposal.yes_votes.len()),
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // 5. EXECUTE PROPOSAL
    // -----------------------------------------------------------------------

    /// If quorum is met, transfer the requested funds to the admin and
    /// mark the proposal as Executed.
    ///
    /// MVP quorum: at least `QUORUM_MIN` (= 1) YES vote.
    ///
    /// # Arguments
    /// * `caller`      – Must be the admin.
    /// * `proposal_id` – Proposal to execute.
    pub fn execute_proposal(
        env: Env,
        caller: Address,
        proposal_id: u32,
    ) -> Result<(), FundError> {
        // Only the admin may trigger execution.
        let admin: Address = env.storage().instance()
            .get(&DataKey::Admin)
            .unwrap();
        if caller != admin {
            return Err(FundError::Unauthorized);
        }
        caller.require_auth();

        // Load the proposal.
        let mut proposal: Proposal = env.storage().persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(FundError::ProposalNotFound)?;

        if proposal.status != ProposalStatus::Pending {
            return Err(FundError::ProposalNotPending);
        }

        // Check quorum.
        let yes_count = proposal.yes_votes.len();
        if yes_count < QUORUM_MIN {
            return Err(FundError::QuorumNotReached);
        }

        // Verify the vault can cover the withdrawal.
        let token_addr: Address = env.storage().instance()
            .get(&DataKey::TokenAddress)
            .unwrap();
        let token_client = token::Client::new(&env, &token_addr);
        let vault_balance = token_client
            .balance(&env.current_contract_address());
        if vault_balance < proposal.amount {
            return Err(FundError::InsufficientFunds);
        }

        // Transfer funds from vault → admin.
        token_client.transfer(
            &env.current_contract_address(),
            &admin,
            &proposal.amount,
        );

        // Close out the proposal.
        proposal.status = ProposalStatus::Executed;
        env.storage().persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish(
            (symbol_short!("execute"), symbol_short!("prop")),
            (proposal_id, proposal.amount, yes_count),
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // READ-ONLY HELPERS (useful for front-end / testing)
    // -----------------------------------------------------------------------

    /// Returns the stored admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    /// Returns the campaign name.
    pub fn get_name(env: Env) -> String {
        env.storage().instance().get(&DataKey::CampaignName).unwrap()
    }

    /// Returns how much a specific donor has contributed (lifetime total).
    pub fn get_donor_balance(env: Env, donor: Address) -> i128 {
        env.storage().persistent()
            .get(&DataKey::DonorBalance(donor))
            .unwrap_or(0)
    }

    /// Returns the full list of registered donor addresses.
    pub fn get_donors(env: Env) -> Vec<Address> {
        env.storage().persistent()
            .get(&DataKey::DonorList)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Returns a proposal by its ID.
    pub fn get_proposal(env: Env, proposal_id: u32) -> Result<Proposal, FundError> {
        env.storage().persistent()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(FundError::ProposalNotFound)
    }

    /// Returns the total number of proposals ever created.
    pub fn get_proposal_count(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::ProposalCount).unwrap_or(0)
    }

    /// Returns the live token balance held in the contract vault.
    pub fn get_vault_balance(env: Env) -> i128 {
        let token_addr: Address = env.storage().instance()
            .get(&DataKey::TokenAddress)
            .unwrap();
        token::Client::new(&env, &token_addr)
            .balance(&env.current_contract_address())
    }
}