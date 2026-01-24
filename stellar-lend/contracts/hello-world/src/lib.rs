#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Map, String, Symbol, Vec};

mod deposit;
mod risk_management;

use deposit::deposit_collateral;
use risk_management::{
    can_be_liquidated, get_close_factor, get_liquidation_incentive,
    get_liquidation_incentive_amount, get_liquidation_threshold, get_max_liquidatable_amount,
    get_min_collateral_ratio, initialize_risk_management, is_emergency_paused, is_operation_paused,
    require_min_collateral_ratio, set_emergency_pause, set_pause_switch, set_pause_switches,
    set_risk_params, RiskConfig, RiskManagementError,
};

mod withdraw;
use withdraw::withdraw_collateral;

mod repay;
use repay::repay_debt;

mod borrow;
use borrow::borrow_asset;

mod governance;
use governance::{
    approve_proposal, create_proposal, execute_multisig_proposal, execute_proposal,
    get_multisig_admins, get_multisig_threshold, get_proposal, get_proposal_approvals, get_vote,
    initialize_governance, mark_proposal_failed, propose_set_min_collateral_ratio, set_multisig_admins,
    set_multisig_threshold, GovernanceError, Proposal, ProposalStatus, ProposalType, Vote,
};

#[contract]
pub struct HelloContract;

#[contractimpl]
impl HelloContract {
    pub fn hello(env: Env) -> String {
        String::from_str(&env, "Hello")
    }

    /// Initialize the contract with admin address
    ///
    /// Sets up the risk management system with default parameters.
    /// Must be called before any other operations.
    ///
    /// # Arguments
    /// * `admin` - The admin address
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn initialize(env: Env, admin: Address) -> Result<(), RiskManagementError> {
        initialize_risk_management(&env, admin.clone())?;
        initialize_governance(&env, admin).map_err(|_| RiskManagementError::Unauthorized)?;
        Ok(())
    }

    /// Deposit collateral into the protocol
    ///
    /// Allows users to deposit assets as collateral in the protocol.
    /// Supports multiple asset types including XLM (native) and token contracts (USDC, etc.).
    ///
    /// # Arguments
    /// * `user` - The address of the user depositing collateral
    /// * `asset` - The address of the asset contract to deposit (None for native XLM)
    /// * `amount` - The amount to deposit
    ///
    /// # Returns
    /// Returns the updated collateral balance for the user
    ///
    /// # Events
    /// Emits the following events:
    /// - `deposit`: Deposit transaction event
    /// - `position_updated`: User position update event
    /// - `analytics_updated`: Analytics update event
    /// - `user_activity_tracked`: User activity tracking event
    pub fn deposit_collateral(
        env: Env,
        user: Address,
        asset: Option<Address>,
        amount: i128,
    ) -> i128 {
        deposit_collateral(&env, user, asset, amount)
            .unwrap_or_else(|e| panic!("Deposit error: {:?}", e))
    }

    /// Set risk parameters (admin only)
    ///
    /// Updates risk parameters with validation and change limits.
    ///
    /// # Arguments
    /// * `caller` - The caller address (must be admin)
    /// * `min_collateral_ratio` - Optional new minimum collateral ratio (in basis points)
    /// * `liquidation_threshold` - Optional new liquidation threshold (in basis points)
    /// * `close_factor` - Optional new close factor (in basis points)
    /// * `liquidation_incentive` - Optional new liquidation incentive (in basis points)
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn set_risk_params(
        env: Env,
        caller: Address,
        min_collateral_ratio: Option<i128>,
        liquidation_threshold: Option<i128>,
        close_factor: Option<i128>,
        liquidation_incentive: Option<i128>,
    ) -> Result<(), RiskManagementError> {
        set_risk_params(
            &env,
            caller,
            min_collateral_ratio,
            liquidation_threshold,
            close_factor,
            liquidation_incentive,
        )
    }

    /// Set pause switch for an operation (admin only)
    ///
    /// # Arguments
    /// * `caller` - The caller address (must be admin)
    /// * `operation` - The operation symbol (e.g., "pause_deposit", "pause_borrow")
    /// * `paused` - Whether to pause (true) or unpause (false)
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn set_pause_switch(
        env: Env,
        caller: Address,
        operation: Symbol,
        paused: bool,
    ) -> Result<(), RiskManagementError> {
        set_pause_switch(&env, caller, operation, paused)
    }

    /// Set multiple pause switches at once (admin only)
    ///
    /// # Arguments
    /// * `caller` - The caller address (must be admin)
    /// * `switches` - Map of operation symbols to pause states
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn set_pause_switches(
        env: Env,
        caller: Address,
        switches: Map<Symbol, bool>,
    ) -> Result<(), RiskManagementError> {
        set_pause_switches(&env, caller, switches)
    }

    /// Set emergency pause (admin only)
    ///
    /// Emergency pause stops all operations immediately.
    ///
    /// # Arguments
    /// * `caller` - The caller address (must be admin)
    /// * `paused` - Whether to enable (true) or disable (false) emergency pause
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn set_emergency_pause(
        env: Env,
        caller: Address,
        paused: bool,
    ) -> Result<(), RiskManagementError> {
        set_emergency_pause(&env, caller, paused)
    }

    /// Get current risk configuration
    ///
    /// # Returns
    /// Returns the current risk configuration or None if not initialized
    pub fn get_risk_config(env: Env) -> Option<RiskConfig> {
        risk_management::get_risk_config(&env)
    }

    /// Get minimum collateral ratio
    ///
    /// # Returns
    /// Returns the minimum collateral ratio in basis points
    pub fn get_min_collateral_ratio(env: Env) -> Result<i128, RiskManagementError> {
        get_min_collateral_ratio(&env)
    }

    /// Get liquidation threshold
    ///
    /// # Returns
    /// Returns the liquidation threshold in basis points
    pub fn get_liquidation_threshold(env: Env) -> Result<i128, RiskManagementError> {
        get_liquidation_threshold(&env)
    }

    /// Get close factor
    ///
    /// # Returns
    /// Returns the close factor in basis points
    pub fn get_close_factor(env: Env) -> Result<i128, RiskManagementError> {
        get_close_factor(&env)
    }

    /// Get liquidation incentive
    ///
    /// # Returns
    /// Returns the liquidation incentive in basis points
    pub fn get_liquidation_incentive(env: Env) -> Result<i128, RiskManagementError> {
        get_liquidation_incentive(&env)
    }

    /// Check if an operation is paused
    ///
    /// # Arguments
    /// * `operation` - The operation symbol to check
    ///
    /// # Returns
    /// Returns true if the operation is paused
    pub fn is_operation_paused(env: Env, operation: Symbol) -> bool {
        is_operation_paused(&env, operation)
    }

    /// Check if emergency pause is active
    ///
    /// # Returns
    /// Returns true if emergency pause is active
    pub fn is_emergency_paused(env: Env) -> bool {
        is_emergency_paused(&env)
    }

    /// Check if user meets minimum collateral ratio requirement
    ///
    /// # Arguments
    /// * `collateral_value` - Total collateral value (in base units)
    /// * `debt_value` - Total debt value (in base units)
    ///
    /// # Returns
    /// Returns Ok(()) if ratio is sufficient, Err otherwise
    pub fn require_min_collateral_ratio(
        env: Env,
        collateral_value: i128,
        debt_value: i128,
    ) -> Result<(), RiskManagementError> {
        require_min_collateral_ratio(&env, collateral_value, debt_value)
    }

    /// Check if position can be liquidated
    ///
    /// # Arguments
    /// * `collateral_value` - Total collateral value (in base units)
    /// * `debt_value` - Total debt value (in base units)
    ///
    /// # Returns
    /// Returns true if position can be liquidated
    pub fn can_be_liquidated(
        env: Env,
        collateral_value: i128,
        debt_value: i128,
    ) -> Result<bool, RiskManagementError> {
        can_be_liquidated(&env, collateral_value, debt_value)
    }

    /// Calculate maximum liquidatable amount
    ///
    /// # Arguments
    /// * `debt_value` - Total debt value (in base units)
    ///
    /// # Returns
    /// Maximum amount that can be liquidated
    pub fn get_max_liquidatable_amount(
        env: Env,
        debt_value: i128,
    ) -> Result<i128, RiskManagementError> {
        get_max_liquidatable_amount(&env, debt_value)
    }

    /// Calculate liquidation incentive amount
    ///
    /// # Arguments
    /// * `liquidated_amount` - Amount being liquidated (in base units)
    ///
    /// # Returns
    /// Liquidation incentive amount
    pub fn get_liquidation_incentive_amount(
        env: Env,
        liquidated_amount: i128,
    ) -> Result<i128, RiskManagementError> {
        get_liquidation_incentive_amount(&env, liquidated_amount)
    }

    /// Withdraw collateral from the protocol
    ///
    /// Allows users to withdraw their deposited collateral, subject to:
    /// - Sufficient collateral balance
    /// - Minimum collateral ratio requirements
    /// - Pause switch checks
    ///
    /// # Arguments
    /// * `user` - The address of the user withdrawing collateral
    /// * `asset` - The address of the asset contract to withdraw (None for native XLM)
    /// * `amount` - The amount to withdraw
    ///
    /// # Returns
    /// Returns the updated collateral balance for the user
    ///
    /// # Events
    /// Emits the following events:
    /// - `withdraw`: Withdraw transaction event
    /// - `position_updated`: User position update event
    /// - `analytics_updated`: Analytics update event
    /// - `user_activity_tracked`: User activity tracking event
    pub fn withdraw_collateral(
        env: Env,
        user: Address,
        asset: Option<Address>,
        amount: i128,
    ) -> i128 {
        withdraw_collateral(&env, user, asset, amount)
            .unwrap_or_else(|e| panic!("Withdraw error: {:?}", e))
    }

    /// Repay debt to the protocol
    ///
    /// Allows users to repay their borrowed assets, reducing debt and accrued interest.
    /// Supports both partial and full repayments.
    ///
    /// # Arguments
    /// * `user` - The address of the user repaying debt
    /// * `asset` - The address of the asset contract to repay (None for native XLM)
    /// * `amount` - The amount to repay
    ///
    /// # Returns
    /// Returns a tuple (remaining_debt, interest_paid, principal_paid)
    ///
    /// # Events
    /// Emits the following events:
    /// - `repay`: Repay transaction event
    /// - `position_updated`: User position update event
    /// - `analytics_updated`: Analytics update event
    /// - `user_activity_tracked`: User activity tracking event
    pub fn repay_debt(
        env: Env,
        user: Address,
        asset: Option<Address>,
        amount: i128,
    ) -> (i128, i128, i128) {
        repay_debt(&env, user, asset, amount).unwrap_or_else(|e| panic!("Repay error: {:?}", e))
    }

    /// Borrow assets from the protocol
    ///
    /// Allows users to borrow assets against their deposited collateral, subject to:
    /// - Sufficient collateral balance
    /// - Minimum collateral ratio requirements
    /// - Pause switch checks
    /// - Maximum borrow limits
    ///
    /// # Arguments
    /// * `user` - The address of the user borrowing assets
    /// * `asset` - The address of the asset contract to borrow (None for native XLM)
    /// * `amount` - The amount to borrow
    ///
    /// # Returns
    /// Returns the updated total debt (principal + interest) for the user
    ///
    /// # Events
    /// Emits the following events:
    /// - `borrow`: Borrow transaction event
    /// - `position_updated`: User position update event
    /// - `analytics_updated`: Analytics update event
    /// - `user_activity_tracked`: User activity tracking event
    pub fn borrow_asset(env: Env, user: Address, asset: Option<Address>, amount: i128) -> i128 {
        borrow_asset(&env, user, asset, amount).unwrap_or_else(|e| panic!("Borrow error: {:?}", e))
    }

    // ============================================================================
    // Governance Functions
    // ============================================================================

    /// Create a new governance proposal
    ///
    /// # Arguments
    /// * `proposer` - The address creating the proposal
    /// * `proposal_type` - The type of proposal
    /// * `description` - Description of the proposal
    /// * `voting_period` - Optional voting period in seconds (default: 7 days)
    /// * `execution_timelock` - Optional execution timelock in seconds (default: 2 days)
    /// * `voting_threshold` - Optional voting threshold in basis points (default: 50%)
    ///
    /// # Returns
    /// Returns the proposal ID on success
    pub fn gov_create_proposal(
        env: Env,
        proposer: Address,
        proposal_type: ProposalType,
        description: Symbol,
        voting_period: Option<u64>,
        execution_timelock: Option<u64>,
        voting_threshold: Option<i128>,
    ) -> Result<u64, GovernanceError> {
        create_proposal(
            &env,
            proposer,
            proposal_type,
            description,
            voting_period,
            execution_timelock,
            voting_threshold,
        )
    }

    /// Vote on a proposal
    ///
    /// # Arguments
    /// * `voter` - The address voting
    /// * `proposal_id` - The proposal ID
    /// * `vote` - The vote (For, Against, or Abstain)
    /// * `voting_power` - The voting power of the voter
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn gov_vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        vote: Vote,
        voting_power: i128,
    ) -> Result<(), GovernanceError> {
        governance::vote(&env, voter, proposal_id, vote, voting_power)
    }

    /// Execute a proposal
    ///
    /// # Arguments
    /// * `executor` - The address executing the proposal
    /// * `proposal_id` - The proposal ID
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn gov_execute_proposal(
        env: Env,
        executor: Address,
        proposal_id: u64,
    ) -> Result<(), GovernanceError> {
        execute_proposal(&env, executor, proposal_id)
    }

    /// Mark a proposal as failed
    ///
    /// # Arguments
    /// * `proposal_id` - The proposal ID
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn gov_mark_proposal_failed(env: Env, proposal_id: u64) -> Result<(), GovernanceError> {
        mark_proposal_failed(&env, proposal_id)
    }

    /// Get a proposal
    ///
    /// # Arguments
    /// * `proposal_id` - The proposal ID
    ///
    /// # Returns
    /// Returns the proposal or None if not found
    pub fn gov_get_proposal(env: Env, proposal_id: u64) -> Option<Proposal> {
        get_proposal(&env, proposal_id)
    }

    /// Get a vote for a voter on a proposal
    ///
    /// # Arguments
    /// * `proposal_id` - The proposal ID
    /// * `voter` - The voter address
    ///
    /// # Returns
    /// Returns the vote or None if not found
    pub fn gov_get_vote(env: Env, proposal_id: u64, voter: Address) -> Option<Vote> {
        get_vote(&env, proposal_id, voter)
    }

    // ============================================================================
    // Multisig Functions
    // ============================================================================

    /// Set multisig admins
    ///
    /// # Arguments
    /// * `caller` - The caller address (must be current multisig admin)
    /// * `admins` - The new list of multisig admins
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn ms_set_admins(
        env: Env,
        caller: Address,
        admins: Vec<Address>,
    ) -> Result<(), GovernanceError> {
        set_multisig_admins(&env, caller, admins)
    }

    /// Set multisig threshold
    ///
    /// # Arguments
    /// * `caller` - The caller address (must be multisig admin)
    /// * `threshold` - The number of approvals required
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn ms_set_threshold(
        env: Env,
        caller: Address,
        threshold: u32,
    ) -> Result<(), GovernanceError> {
        set_multisig_threshold(&env, caller, threshold)
    }

    /// Propose setting minimum collateral ratio (multisig)
    ///
    /// # Arguments
    /// * `proposer` - The proposer address (must be multisig admin)
    /// * `new_ratio` - The new minimum collateral ratio in basis points
    ///
    /// # Returns
    /// Returns the proposal ID on success
    pub fn ms_propose_set_min_cr(
        env: Env,
        proposer: Address,
        new_ratio: i128,
    ) -> Result<u64, GovernanceError> {
        propose_set_min_collateral_ratio(&env, proposer, new_ratio)
    }

    /// Approve a multisig proposal
    ///
    /// # Arguments
    /// * `approver` - The approver address (must be multisig admin)
    /// * `proposal_id` - The proposal ID
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn ms_approve(
        env: Env,
        approver: Address,
        proposal_id: u64,
    ) -> Result<(), GovernanceError> {
        approve_proposal(&env, approver, proposal_id)
    }

    /// Execute a multisig proposal
    ///
    /// # Arguments
    /// * `executor` - The executor address (must be multisig admin)
    /// * `proposal_id` - The proposal ID
    ///
    /// # Returns
    /// Returns Ok(()) on success
    pub fn ms_execute(
        env: Env,
        executor: Address,
        proposal_id: u64,
    ) -> Result<(), GovernanceError> {
        execute_multisig_proposal(&env, executor, proposal_id)
    }

    /// Get multisig admins
    ///
    /// # Returns
    /// Returns the list of multisig admins
    pub fn ms_get_admins(env: Env) -> Option<Vec<Address>> {
        get_multisig_admins(&env)
    }

    /// Get multisig threshold
    ///
    /// # Returns
    /// Returns the multisig threshold
    pub fn ms_get_threshold(env: Env) -> u32 {
        get_multisig_threshold(&env)
    }

    /// Get proposal approvals
    ///
    /// # Arguments
    /// * `proposal_id` - The proposal ID
    ///
    /// # Returns
    /// Returns the list of approvers
    pub fn ms_get_approvals(env: Env, proposal_id: u64) -> Option<Vec<Address>> {
        get_proposal_approvals(&env, proposal_id)
    }
}

#[cfg(test)]
mod test;
