use super::*;
use crate::amm::{AmmDataKey, *};
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, Env, Symbol, Vec};

fn create_amm_contract<'a>(env: &Env) -> AmmContractClient<'a> {
    AmmContractClient::new(env, &env.register(AmmContract {}, ()))
}

fn create_test_protocol_config(env: &Env, protocol_addr: &Address) -> AmmProtocolConfig {
    let mut supported_pairs = Vec::new(env);
    supported_pairs.push_back(TokenPair {
        token_a: None,                         // Native XLM
        token_b: Some(Address::generate(env)), // Mock USDC
        pool_address: Address::generate(env),
    });

    AmmProtocolConfig {
        protocol_address: protocol_addr.clone(),
        protocol_name: Symbol::new(env, "TestAMM"),
        enabled: true,
        fee_tier: 30, // 0.3%
        min_swap_amount: 1000,
        max_swap_amount: 1_000_000_000,
        supported_pairs,
    }
}

#[test]
fn test_initialize_amm_settings() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);

    // Initialize AMM settings - this should not panic
    contract.initialize_amm_settings(
        &admin, &100,   // 1% default slippage
        &1000,  // 10% max slippage
        &10000, // 10000 auto-swap threshold
    );

    // Verify settings were stored
    let settings = contract.get_amm_settings();
    assert!(settings.is_some());
    let settings = settings.unwrap();
    assert_eq!(settings.default_slippage, 100);
    assert_eq!(settings.max_slippage, 1000);
    assert_eq!(settings.auto_swap_threshold, 10000);
    assert!(settings.swap_enabled);
    assert!(settings.liquidity_enabled);
}

#[test]
fn test_add_amm_protocol() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    // Initialize first
    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    // Create protocol config
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);

    // Add protocol - this should not panic
    contract.add_amm_protocol(&admin, &protocol_config);

    // Verify protocol was added
    let protocols = contract.get_amm_protocols();
    assert!(protocols.is_some());
    let protocols = protocols.unwrap();
    assert!(protocols.contains_key(protocol_addr.clone()));
}

#[test]
fn test_update_amm_settings() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);

    // Initialize
    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    // Update settings
    let new_settings = AmmSettings {
        default_slippage: 200,
        max_slippage: 2000,
        swap_enabled: false,
        liquidity_enabled: true,
        auto_swap_threshold: 20000,
    };

    contract.update_amm_settings(&admin, &new_settings);

    // Verify settings were updated
    let settings = contract.get_amm_settings().unwrap();
    assert_eq!(settings.default_slippage, 200);
    assert_eq!(settings.max_slippage, 2000);
    assert!(!settings.swap_enabled);
    assert!(settings.liquidity_enabled);
    assert_eq!(settings.auto_swap_threshold, 20000);
}

#[test]
fn test_successful_swap() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);
    let token_b = Address::generate(&env);

    // Initialize
    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    // Register protocol with a pair
    let mut supported_pairs = Vec::new(&env);
    supported_pairs.push_back(TokenPair {
        token_a: None, // Native XLM
        token_b: Some(token_b.clone()),
        pool_address: Address::generate(&env),
    });

    let protocol_config = AmmProtocolConfig {
        protocol_address: protocol_addr.clone(),
        protocol_name: Symbol::new(&env, "TestAMM"),
        enabled: true,
        fee_tier: 30,
        min_swap_amount: 1000,
        max_swap_amount: 1_000_000_000,
        supported_pairs,
    };
    contract.add_amm_protocol(&admin, &protocol_config);

    // Execute swap
    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: Some(token_b.clone()),
        amount_in: 10000,
        min_amount_out: 9000,
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    let amount_out = contract.execute_swap(&user, &params);
    assert_eq!(amount_out, 9900); // 10000 * (10000 - 100) / 10000 = 9900 based on mock execute_amm_swap

    // Verify swap history
    let history = contract.get_swap_history(&Some(user), &10).unwrap();
    assert_eq!(history.len(), 1);
    let record = history.get(0).unwrap();
    assert_eq!(record.amount_in, 10000);
    assert_eq!(record.amount_out, 9900);
}

#[test]
fn test_swap_failure_insufficient_output() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: protocol_config.supported_pairs.get(0).unwrap().token_b,
        amount_in: 10000,
        min_amount_out: 10000, // Too high for 1% mock slippage
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_execute_swap(&user, &params);
    assert!(result.is_err());
}

#[test]
fn test_swap_failure_deadline_exceeded() {
    let env = Env::default();
    env.mock_all_auths();

    // Set a known timestamp
    env.ledger().set_timestamp(1000);

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: protocol_config.supported_pairs.get(0).unwrap().token_b,
        amount_in: 10000,
        min_amount_out: 5000,
        slippage_tolerance: 100,
        deadline: 999, // Before current ledger timestamp (1000)
    };

    let result = contract.try_execute_swap(&user, &params);
    assert!(result.is_err());
}

#[test]
fn test_swap_failure_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let mut settings = contract.get_amm_settings().unwrap();
    settings.swap_enabled = false;
    contract.update_amm_settings(&admin, &settings);

    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: protocol_config.supported_pairs.get(0).unwrap().token_b,
        amount_in: 10000,
        min_amount_out: 5000,
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_execute_swap(&user, &params);
    assert!(result.is_err());
}

#[test]
fn test_add_liquidity() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);
    let token_b = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    let mut supported_pairs = Vec::new(&env);
    supported_pairs.push_back(TokenPair {
        token_a: None,
        token_b: Some(token_b.clone()),
        pool_address: Address::generate(&env),
    });

    let protocol_config = AmmProtocolConfig {
        protocol_address: protocol_addr.clone(),
        protocol_name: Symbol::new(&env, "TestAMM"),
        enabled: true,
        fee_tier: 30,
        min_swap_amount: 1000,
        max_swap_amount: 1_000_000_000,
        supported_pairs,
    };
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = LiquidityParams {
        protocol: protocol_addr.clone(),
        token_a: None,
        token_b: Some(token_b.clone()),
        amount_a: 10000,
        amount_b: 10000,
        min_amount_a: 9000,
        min_amount_b: 9000,
        deadline: env.ledger().timestamp() + 3600,
    };

    let lp_tokens = contract.add_liquidity(&user, &params);
    assert_eq!(lp_tokens, 10000);

    let history = contract.get_liquidity_history(&Some(user), &10).unwrap();
    assert_eq!(history.len(), 1);
}

#[test]
fn test_remove_liquidity() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);
    let token_b = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let mut supported_pairs = Vec::new(&env);
    supported_pairs.push_back(TokenPair {
        token_a: None,
        token_b: Some(token_b.clone()),
        pool_address: Address::generate(&env),
    });
    let protocol_config = AmmProtocolConfig {
        protocol_address: protocol_addr.clone(),
        protocol_name: Symbol::new(&env, "TestAMM"),
        enabled: true,
        fee_tier: 30,
        min_swap_amount: 1000,
        max_swap_amount: 1_000_000_000,
        supported_pairs,
    };
    contract.add_amm_protocol(&admin, &protocol_config);

    let (amount_a, amount_b) = contract.remove_liquidity(
        &user,
        &protocol_addr,
        &None,
        &Some(token_b.clone()),
        &5000,
        &4000,
        &4000,
        &(env.ledger().timestamp() + 3600),
    );

    assert_eq!(amount_a, 5000);
    assert_eq!(amount_b, 5000);

    let history = contract.get_liquidity_history(&Some(user), &10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(
        history.get(0).unwrap().operation_type,
        Symbol::new(&env, "remove")
    );
}

#[test]
fn test_callback_validation() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let callback_data = AmmCallbackData {
        nonce: 999, // Wrong nonce
        operation: Symbol::new(&env, "swap"),
        user: user.clone(),
        expected_amounts: Vec::new(&env),
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_validate_amm_callback(&protocol_addr, &callback_data);
    assert!(result.is_err());
}

#[test]
fn test_auto_swap_for_collateral() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);
    let token_out = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    let mut supported_pairs = Vec::new(&env);
    supported_pairs.push_back(TokenPair {
        token_a: None,
        token_b: Some(token_out.clone()),
        pool_address: Address::generate(&env),
    });
    let protocol_config = AmmProtocolConfig {
        protocol_address: protocol_addr.clone(),
        protocol_name: Symbol::new(&env, "BestAMM"),
        enabled: true,
        fee_tier: 30,
        min_swap_amount: 1000,
        max_swap_amount: 1_000_000_000,
        supported_pairs,
    };
    contract.add_amm_protocol(&admin, &protocol_config);

    let amount_out = contract.auto_swap_for_collateral(&user, &Some(token_out), &15000);
    assert_eq!(amount_out, 14850);
}

#[test]
fn test_swap_failure_unsupported_protocol() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: Some(Address::generate(&env)),
        amount_in: 10000,
        min_amount_out: 5000,
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_execute_swap(&user, &params);
    assert!(result.is_err());
}

#[test]
fn test_swap_failure_invalid_token_pair() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: Some(Address::generate(&env)), // Not in supported pairs
        token_out: Some(Address::generate(&env)),
        amount_in: 10000,
        min_amount_out: 5000,
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_execute_swap(&user, &params);
    assert!(result.is_err());
}

#[test]
fn test_liquidity_failure_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let mut settings = contract.get_amm_settings().unwrap();
    settings.liquidity_enabled = false;
    contract.update_amm_settings(&admin, &settings);

    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = LiquidityParams {
        protocol: protocol_addr.clone(),
        token_a: None,
        token_b: protocol_config.supported_pairs.get(0).unwrap().token_b,
        amount_a: 10000,
        amount_b: 10000,
        min_amount_a: 5000,
        min_amount_b: 5000,
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_add_liquidity(&user, &params);
    assert!(result.is_err());
}

#[test]
fn test_get_history_with_limit() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);
    let _token_b = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    // Perform 3 swaps
    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: protocol_config.supported_pairs.get(0).unwrap().token_b,
        amount_in: 10000,
        min_amount_out: 5000,
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    contract.execute_swap(&user, &params);
    contract.execute_swap(&user, &params);
    contract.execute_swap(&user, &params);

    // Get history with limit 2
    let history = contract.get_swap_history(&Some(user), &2).unwrap();
    assert_eq!(history.len(), 2);
}

#[test]
fn test_multiple_protocol_selection() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let token_out = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    // Protocol 1: Disabled
    let protocol1 = Address::generate(&env);
    let mut config1 = create_test_protocol_config(&env, &protocol1);
    config1.enabled = false;
    contract.add_amm_protocol(&admin, &config1);

    // Protocol 2: Enabled but doesn't support the pair
    let protocol2 = Address::generate(&env);
    let mut config2 = create_test_protocol_config(&env, &protocol2);
    config2.supported_pairs = Vec::new(&env); // No pairs supported
    contract.add_amm_protocol(&admin, &config2);

    // Protocol 3: Enabled and supports the pair
    let protocol3 = Address::generate(&env);
    let mut supported_pairs = Vec::new(&env);
    supported_pairs.push_back(TokenPair {
        token_a: None,
        token_b: Some(token_out.clone()),
        pool_address: Address::generate(&env),
    });
    let config3 = AmmProtocolConfig {
        protocol_address: protocol3.clone(),
        protocol_name: Symbol::new(&env, "WorkingAMM"),
        enabled: true,
        fee_tier: 30,
        min_swap_amount: 1000,
        max_swap_amount: 1_000_000_000,
        supported_pairs,
    };
    contract.add_amm_protocol(&admin, &config3);

    // Should pick Protocol 3
    let amount_out = contract.auto_swap_for_collateral(&user, &Some(token_out), &15000);
    assert_eq!(amount_out, 14850);
}

#[test]
fn test_swap_failure_max_input_exceeded() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let mut protocol_config = create_test_protocol_config(&env, &protocol_addr);
    protocol_config.max_swap_amount = 5000;
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: protocol_config.supported_pairs.get(0).unwrap().token_b,
        amount_in: 10000, // Exceeds max
        min_amount_out: 5000,
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_execute_swap(&user, &params);
    assert!(result.is_err());
}

#[test]
fn test_swap_failure_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: protocol_config.supported_pairs.get(0).unwrap().token_b,
        amount_in: 0,
        min_amount_out: 5000,
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_execute_swap(&user, &params);
    assert!(result.is_err());
}

#[test]
fn test_admin_only_operations() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    let new_settings = AmmSettings {
        default_slippage: 200,
        max_slippage: 2000,
        swap_enabled: true,
        liquidity_enabled: true,
        auto_swap_threshold: 20000,
    };

    let result = contract.try_update_amm_settings(&non_admin, &new_settings);
    assert!(result.is_err());
}

#[test]
fn test_callback_validation_expired() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let callback_data = AmmCallbackData {
        nonce: 1,
        operation: Symbol::new(&env, "swap"),
        user: user.clone(),
        expected_amounts: Vec::new(&env),
        deadline: 500, // Past deadline
    };

    env.ledger().set_timestamp(1000);

    let result = contract.try_validate_amm_callback(&protocol_addr, &callback_data);
    assert!(result.is_err());
}

#[test]
fn test_callback_validation_success() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);
    let token_b = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    let mut supported_pairs = Vec::new(&env);
    supported_pairs.push_back(TokenPair {
        token_a: None,
        token_b: Some(token_b.clone()),
        pool_address: Address::generate(&env),
    });
    let protocol_config = AmmProtocolConfig {
        protocol_address: protocol_addr.clone(),
        protocol_name: Symbol::new(&env, "Test"),
        enabled: true,
        fee_tier: 30,
        min_swap_amount: 10,
        max_swap_amount: 1000000,
        supported_pairs,
    };
    contract.add_amm_protocol(&admin, &protocol_config);

    // Trigger an operation to increment nonce
    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: Some(token_b.clone()),
        amount_in: 1000,
        min_amount_out: 100,
        slippage_tolerance: 100,
        deadline: 2000,
    };
    env.ledger().set_timestamp(1000);
    contract.execute_swap(&user, &params);

    // `execute_swap` allocates nonce 1 and `validate_amm_callback_core` consumes it, leaving stored nonce 2.
    let callback_data = AmmCallbackData {
        nonce: 2,
        operation: Symbol::new(&env, "swap"),
        user: user.clone(),
        expected_amounts: Vec::new(&env),
        deadline: 2000,
    };

    contract.validate_amm_callback(&protocol_addr, &callback_data);
}

#[test]
fn test_validate_amm_callback_fails_without_caller_auth() {
    let env = Env::default();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let callback_data = AmmCallbackData {
        nonce: 0,
        operation: Symbol::new(&env, "swap"),
        user: user.clone(),
        expected_amounts: Vec::new(&env),
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_validate_amm_callback(&protocol_addr, &callback_data);
    assert!(result.is_err());
}

#[test]
fn test_validate_amm_callback_succeeds_with_caller_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let callback_data = AmmCallbackData {
        nonce: 0,
        operation: Symbol::new(&env, "swap"),
        user: user.clone(),
        expected_amounts: Vec::new(&env),
        deadline: env.ledger().timestamp() + 3600,
    };

    assert!(contract
        .try_validate_amm_callback(&protocol_addr, &callback_data)
        .unwrap()
        .is_ok());
}

#[test]
fn test_callback_replay_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let callback_data = AmmCallbackData {
        nonce: 0,
        operation: Symbol::new(&env, "swap"),
        user: user.clone(),
        expected_amounts: Vec::new(&env),
        deadline: env.ledger().timestamp() + 3600,
    };

    assert!(contract
        .try_validate_amm_callback(&protocol_addr, &callback_data)
        .unwrap()
        .is_ok());
    let replay = contract.try_validate_amm_callback(&protocol_addr, &callback_data);
    assert!(replay.is_err());
}

#[test]
fn test_callback_disabled_protocol_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let mut protocol_config = create_test_protocol_config(&env, &protocol_addr);
    protocol_config.enabled = false;
    contract.add_amm_protocol(&admin, &protocol_config);

    let callback_data = AmmCallbackData {
        nonce: 0,
        operation: Symbol::new(&env, "swap"),
        user: user.clone(),
        expected_amounts: Vec::new(&env),
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_validate_amm_callback(&protocol_addr, &callback_data);
    assert!(result.is_err());
}

#[test]
fn test_callback_unregistered_protocol_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);
    let other_protocol = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    let callback_data = AmmCallbackData {
        nonce: 0,
        operation: Symbol::new(&env, "swap"),
        user: user.clone(),
        expected_amounts: Vec::new(&env),
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_validate_amm_callback(&other_protocol, &callback_data);
    assert!(result.is_err());
}

#[test]
fn test_callback_nonce_overflow_on_increment() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let protocol_config = create_test_protocol_config(&env, &protocol_addr);
    contract.add_amm_protocol(&admin, &protocol_config);

    env.as_contract(&contract.address, || {
        let nonce_key = AmmDataKey::CallbackNonces(user.clone());
        env.storage().persistent().set(&nonce_key, &u64::MAX);
    });

    let callback_data = AmmCallbackData {
        nonce: u64::MAX,
        operation: Symbol::new(&env, "swap"),
        user: user.clone(),
        expected_amounts: Vec::new(&env),
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_validate_amm_callback(&protocol_addr, &callback_data);
    assert!(result.is_err());
}

#[test]
fn test_generate_callback_nonce_overflow_on_swap() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);
    let token_b = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);

    let mut supported_pairs = Vec::new(&env);
    supported_pairs.push_back(TokenPair {
        token_a: None,
        token_b: Some(token_b.clone()),
        pool_address: Address::generate(&env),
    });
    let protocol_config = AmmProtocolConfig {
        protocol_address: protocol_addr.clone(),
        protocol_name: Symbol::new(&env, "Test"),
        enabled: true,
        fee_tier: 30,
        min_swap_amount: 10,
        max_swap_amount: 1000000,
        supported_pairs,
    };
    contract.add_amm_protocol(&admin, &protocol_config);

    env.as_contract(&contract.address, || {
        let nonce_key = AmmDataKey::CallbackNonces(user.clone());
        env.storage().persistent().set(&nonce_key, &u64::MAX);
    });

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: Some(token_b.clone()),
        amount_in: 1000,
        min_amount_out: 100,
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_execute_swap(&user, &params);
    assert!(result.is_err());
}

#[test]
fn test_edge_case_max_slippage() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);
    let token_b = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &2000, &10000); // 20% max slippage allowed

    let mut supported_pairs = Vec::new(&env);
    supported_pairs.push_back(TokenPair {
        token_a: None,
        token_b: Some(token_b.clone()),
        pool_address: Address::generate(&env),
    });
    let protocol_config = AmmProtocolConfig {
        protocol_address: protocol_addr.clone(),
        protocol_name: Symbol::new(&env, "Test"),
        enabled: true,
        fee_tier: 30,
        min_swap_amount: 1,
        max_swap_amount: 1000000,
        supported_pairs,
    };
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: Some(token_b.clone()),
        amount_in: 10000,
        min_amount_out: 1,        // High slippage tolerance
        slippage_tolerance: 2000, // 20%
        deadline: 2000,
    };
    env.ledger().set_timestamp(1000);
    let amount_out = contract.execute_swap(&user, &params);
    assert!(amount_out > 0);
}

#[test]
fn test_edge_case_min_swap_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = create_amm_contract(&env);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let protocol_addr = Address::generate(&env);

    contract.initialize_amm_settings(&admin, &100, &1000, &10000);
    let mut protocol_config = create_test_protocol_config(&env, &protocol_addr);
    protocol_config.min_swap_amount = 5000;
    contract.add_amm_protocol(&admin, &protocol_config);

    let params = SwapParams {
        protocol: protocol_addr.clone(),
        token_in: None,
        token_out: protocol_config.supported_pairs.get(0).unwrap().token_b,
        amount_in: 1000, // Below min
        min_amount_out: 100,
        slippage_tolerance: 100,
        deadline: env.ledger().timestamp() + 3600,
    };

    let result = contract.try_execute_swap(&user, &params);
    assert!(result.is_err());
}
